//! Experiment executor.
//!
//! [`Experiment`] walks an [`ExperimentConfig`]'s steps in order, threading
//! each step's output into the next step's input via a [`StepContext`].

use crate::parser::{DnsStep, DnsTransport, ExperimentConfig, HttpVersion, Step, TcpStep};
use bytes::Bytes;
use http::{Method, Request};
use http_body_util::Full;
use ooniprobe_network::{
    dns::TracingResolver,
    errors::OoniError,
    http::{get_request, http_user_agent, TracingHttpClient},
    tcp::{TracingDialer, TracingStream},
    tls::TracingTlsHandshaker,
    trace::Trace,
};
use std::net::{IpAddr, SocketAddr};
use tokio_rustls::client::TlsStream;

/// Streams the executor holds between steps.
struct StepContext {
    resolved_ips: Vec<IpAddr>,
    dns_hostname: Option<String>,
    tcp_streams: Vec<(SocketAddr, TracingStream)>,
    tls_streams: Vec<(SocketAddr, TlsStream<TracingStream>)>,
}

impl StepContext {
    fn new() -> Self {
        Self {
            resolved_ips: vec![],
            dns_hostname: None,
            tcp_streams: vec![],
            tls_streams: vec![],
        }
    }

    fn clear_streams(&mut self) {
        self.tcp_streams.clear();
        self.tls_streams.clear();
    }
}

fn parse_addr(server: Option<&str>, default_port: u16) -> Result<SocketAddr, OoniError> {
    let s = server.ok_or_else(|| OoniError::Unknown("missing server field".into()))?;
    if let Ok(sa) = s.parse::<SocketAddr>() {
        return Ok(sa);
    }
    if let Ok(ip) = s.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, default_port));
    }
    Err(OoniError::Unknown(format!("cannot parse address '{s}'")))
}

pub struct Experiment {
    cfg: ExperimentConfig,
    trace: Trace,
}

fn explicit_to_addr(s: &str, fallback_port: u16) -> Result<SocketAddr, OoniError> {
    if let Ok(sa) = s.parse::<SocketAddr>() {
        return Ok(sa);
    }
    if let Ok(ip) = s.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, fallback_port));
    }
    Err(OoniError::Unknown(format!(
        "cannot parse tcp address '{s}'"
    )))
}

fn alpn_for(version: &HttpVersion) -> &'static str {
    match version {
        HttpVersion::H1 => "http/1.1",
        HttpVersion::H2 | HttpVersion::Auto => "h2",
    }
}

fn build_request(url: &str, method: &str) -> Result<Request<Full<Bytes>>, OoniError> {
    if method.eq_ignore_ascii_case("GET") {
        return get_request(url);
    }
    let m = Method::from_bytes(method.as_bytes())
        .map_err(|e| OoniError::Unknown(format!("invalid HTTP method '{method}': {e}")))?;
    let uri: http::Uri = url
        .parse()
        .map_err(|e| OoniError::Unknown(format!("invalid URL '{url}': {e}")))?;
    let host = uri
        .host()
        .ok_or_else(|| OoniError::Unknown("URL has no host".into()))?
        .to_owned();
    Request::builder()
        .method(m)
        .uri(uri)
        .header("Host", &host)
        .header("User-Agent", http_user_agent())
        .body(Full::new(Bytes::new()))
        .map_err(|e| OoniError::Unknown(format!("request build error: {e}")))
}

impl Experiment {
    pub fn new(cfg: ExperimentConfig, trace: Trace) -> Self {
        Self { cfg, trace }
    }

    async fn build_resolver(&self, s: &DnsStep) -> Result<TracingResolver, OoniError> {
        match &s.transport {
            DnsTransport::System => Ok(TracingResolver::system(self.trace.clone())),
            DnsTransport::Udp => {
                let addr = parse_addr(s.server.as_deref(), 53)?;
                Ok(TracingResolver::udp(addr, self.trace.clone()))
            }
            DnsTransport::Dot => {
                let addr = parse_addr(s.server.as_deref(), 853)?;
                Ok(TracingResolver::dot(
                    addr,
                    s.server.clone().unwrap_or_default(),
                    self.trace.clone(),
                ))
            }
            DnsTransport::Doh => {
                let url = s
                    .url
                    .as_deref()
                    .ok_or_else(|| OoniError::Unknown("DoH step missing 'url'".into()))?;
                TracingResolver::from_url(url, self.trace.clone()).await
            }
        }
    }

    fn resolve_tcp_addrs(
        &self,
        s: &TcpStep,
        ctx: &StepContext,
    ) -> Result<Vec<SocketAddr>, OoniError> {
        match &s.address {
            Some(explicit) => Ok(vec![explicit_to_addr(explicit, s.port)?]),
            None => Ok(ctx
                .resolved_ips
                .iter()
                .map(|ip| SocketAddr::new(*ip, s.port))
                .collect()),
        }
    }

    /// Execute every step, recording all observations into the trace.
    pub async fn run(self) -> Result<(), OoniError> {
        let mut ctx = StepContext::new();

        for step in &self.cfg.steps {
            match step {
                Step::Dns(s) => {
                    ctx.clear_streams();
                    let resolver = self.build_resolver(s).await?;
                    let ips = resolver.lookup_host(&s.hostname).await?;
                    if ips.is_empty() {
                        return Err(OoniError::DnsNoAnswer);
                    }
                    ctx.resolved_ips = ips;
                    ctx.dns_hostname = Some(s.hostname.clone());
                }

                Step::Tcp(s) => {
                    let addrs = self.resolve_tcp_addrs(s, &ctx)?;
                    let dialer = TracingDialer::new(self.trace.clone());
                    ctx.tcp_streams.clear();
                    ctx.tls_streams.clear();

                    for addr in addrs {
                        let tx_id = self.trace.next_transaction_id();
                        if let Ok(stream) = dialer.connect(addr, tx_id).await {
                            ctx.tcp_streams.push((addr, stream));
                        }
                    }

                    if ctx.tcp_streams.is_empty() {
                        return Err(OoniError::ConnectionRefused);
                    }
                }

                Step::Tls(s) => {
                    let sni = s
                        .sni
                        .clone()
                        .or_else(|| ctx.dns_hostname.clone())
                        .unwrap_or_default();

                    let handshaker = if s.skip_verify {
                        TracingTlsHandshaker::insecure(self.trace.clone())?
                    } else {
                        TracingTlsHandshaker::new(self.trace.clone())?
                    };

                    let tcp_streams = std::mem::take(&mut ctx.tcp_streams);
                    for (addr, tcp) in tcp_streams {
                        let tx_id = self.trace.next_transaction_id();
                        if let Ok(tls) = handshaker
                            .handshake(tcp, &sni, &addr.to_string(), tx_id)
                            .await
                        {
                            ctx.tls_streams.push((addr, tls));
                        }
                    }

                    if ctx.tls_streams.is_empty() {
                        return Err(OoniError::TlsFailedHandshake);
                    }
                }

                Step::Http(s) => {
                    let http = TracingHttpClient::new(self.trace.clone());

                    if !ctx.tls_streams.is_empty() {
                        let streams = std::mem::take(&mut ctx.tls_streams);
                        for (addr, tls) in streams {
                            let tx_id = self.trace.next_transaction_id();
                            if let Ok(req) = build_request(&s.url, &s.method) {
                                let alpn = alpn_for(&s.version);
                                let _ = match s.version {
                                    HttpVersion::H1 => {
                                        http.send_http1(tls, req, &addr.to_string(), alpn, tx_id)
                                            .await
                                    }
                                    HttpVersion::H2 | HttpVersion::Auto => {
                                        http.send_http2(tls, req, &addr.to_string(), alpn, tx_id)
                                            .await
                                    }
                                };
                            }
                        }
                    } else {
                        let streams = std::mem::take(&mut ctx.tcp_streams);
                        for (addr, tcp) in streams {
                            let tx_id = self.trace.next_transaction_id();
                            if let Ok(req) = build_request(&s.url, &s.method) {
                                let _ = http
                                    .send_http1(tcp, req, &addr.to_string(), "http/1.1", tx_id)
                                    .await;
                            }
                        }
                    }
                    // http is terminal — no abort check needed
                }
            }
        }

        Ok(())
    }
}
