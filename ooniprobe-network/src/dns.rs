//! DNS resolution with full transport-layer tracing.
//!
//! Every DNS transport here is built on top of the **same** traced primitives
//! used by the rest of `ooniprobe-network` — [`TracingDialer`], [`TracingTlsHandshaker`],
//! [`TracingHttpClient`], and [`TracingUdpSocket`].
//!
//! Wire encoding/decoding uses `hickory-proto` (no hickory transport layer).

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use bytes::Bytes;
use http::{Method, Request};
use http_body_util::Full;

use hickory_proto::{
    op::{Message, MessageType, OpCode, Query, ResponseCode},
    rr::{DNSClass, Name, RData, RecordType},
    serialize::binary::{BinDecodable, BinEncodable},
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    archival::{DnsAnswer, DnsLookupResult},
    errors::OoniError,
    http::TracingHttpClient,
    tcp::TracingDialer,
    tls::TracingTlsHandshaker,
    trace::Trace,
    udp::TracingUdpSocket,
};

/// Build a DNS query wire message for `name`/`qtype`.
/// Returns `(message_id, wire_bytes)`.
fn build_query(name: &str, qtype: RecordType) -> Result<(u16, Vec<u8>), OoniError> {
    let id: u16 = rand::random();
    let mut msg = Message::new();
    msg.set_id(id)
        .set_message_type(MessageType::Query)
        .set_op_code(OpCode::Query)
        .set_recursion_desired(true);

    let parsed = Name::from_utf8(name)
        .map_err(|e| OoniError::DnsUnexpected(format!("invalid name: {e}")))?;
    let mut q = Query::new();
    q.set_name(parsed)
        .set_query_type(qtype)
        .set_query_class(DNSClass::IN);
    msg.add_query(q);

    let bytes = msg
        .to_vec()
        .map_err(|e| OoniError::DnsUnexpected(format!("encode error: {e}")))?;
    Ok((id, bytes))
}

/// Parse a raw DNS response into `(Vec<DnsAnswer>, rcode)`.
/// Returns `Err` for NXDOMAIN, SERVFAIL, REFUSED, or empty answer sections.
fn parse_response(data: &[u8], qtype: RecordType) -> Result<(Vec<DnsAnswer>, i64), OoniError> {
    let msg = Message::from_bytes(data)
        .map_err(|e| OoniError::DnsUnexpected(format!("decode error: {e}")))?;

    let rcode = msg.response_code() as i64;

    match msg.response_code() {
        ResponseCode::NXDomain => return Err(OoniError::DnsNxDomain),
        ResponseCode::ServFail => return Err(OoniError::DnsServFail),
        ResponseCode::Refused => return Err(OoniError::DnsRefused),
        _ => {}
    }

    let answers: Vec<DnsAnswer> = msg
        .answers()
        .iter()
        .filter_map(|rec| match rec.data() {
            Some(RData::A(a)) if qtype == RecordType::A => Some(DnsAnswer {
                answer_type: "A".into(),
                ipv4: Some(a.to_string()),
                ttl: Some(rec.ttl()),
                ..Default::default()
            }),
            Some(RData::AAAA(a)) if qtype == RecordType::AAAA => Some(DnsAnswer {
                answer_type: "AAAA".into(),
                ipv6: Some(a.to_string()),
                ttl: Some(rec.ttl()),
                ..Default::default()
            }),
            Some(RData::CNAME(c)) => Some(DnsAnswer {
                answer_type: "CNAME".into(),
                hostname: Some(c.to_string()),
                ttl: Some(rec.ttl()),
                ..Default::default()
            }),
            _ => None,
        })
        .collect();

    if answers.is_empty() {
        return Err(OoniError::DnsNoAnswer);
    }

    Ok((answers, rcode))
}

/// Extract IP addresses from a parsed answer list.
fn answers_to_ips(answers: &[DnsAnswer]) -> Vec<IpAddr> {
    answers
        .iter()
        .filter_map(|a| {
            a.ipv4
                .as_deref()
                .and_then(|s| s.parse().ok())
                .map(IpAddr::V4)
                .or_else(|| {
                    a.ipv6
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .map(IpAddr::V6)
                })
        })
        .collect()
}

// DnsTransport — carries the wire message to the resolver
pub enum DnsTransport {
    System,
    Udp {
        server: SocketAddr,
    },
    Dot {
        server: SocketAddr,
        hostname: String,
    },
    Doh {
        server: SocketAddr,
        hostname: String,
        url: String,
    },
}

impl DnsTransport {
    /// Exchange a raw DNS query with the server using our traced primitives.
    /// On return, `trace` is populated with transport-layer events.
    async fn exchange(
        &self,
        query: &[u8],
        trace: &Trace,
        tx_id: i64,
    ) -> Result<Vec<u8>, OoniError> {
        match self {
            Self::System => unreachable!("System transport has no wire exchange"),

            // ── UDP ───────────────────────────────────────────────────────
            Self::Udp { server } => {
                let sock = TracingUdpSocket::connect(*server, trace.clone()).await?;
                sock.exchange(query, Duration::from_secs(5), tx_id).await
            }

            // ── DoT (RFC 7858) ────────────────────────────────────────────
            // 2-byte big-endian length prefix before each DNS message.
            Self::Dot { server, hostname } => {
                let dialer = TracingDialer::new(trace.clone());
                let stream = dialer.connect(*server, tx_id).await?;
                let addr = stream.address().to_owned();

                let hs = TracingTlsHandshaker::new(trace.clone())?;
                let mut tls = hs.handshake(stream, hostname, &addr, tx_id).await?;

                // Send: length-prefixed query.
                let len_bytes = (query.len() as u16).to_be_bytes();
                tls.write_all(&len_bytes)
                    .await
                    .map_err(OoniError::from_io)?;
                tls.write_all(query).await.map_err(OoniError::from_io)?;
                tls.flush().await.map_err(OoniError::from_io)?;

                // Recv: 2-byte length then payload.
                let mut len_buf = [0u8; 2];
                tls.read_exact(&mut len_buf)
                    .await
                    .map_err(OoniError::from_io)?;
                let resp_len = u16::from_be_bytes(len_buf) as usize;
                let mut resp = vec![0u8; resp_len];
                tls.read_exact(&mut resp)
                    .await
                    .map_err(OoniError::from_io)?;

                Ok(resp)
            }

            // ── DoH (RFC 8484) ────────────────────────────────────────────
            // POST /dns-query  Content-Type: application/dns-message
            Self::Doh {
                server,
                hostname,
                url,
            } => {
                let dialer = TracingDialer::new(trace.clone());
                let stream = dialer.connect(*server, tx_id).await?;
                let addr = stream.address().to_owned();

                let hs = TracingTlsHandshaker::new(trace.clone())?;
                let tls = hs.handshake(stream, hostname, &addr, tx_id).await?;

                let http = TracingHttpClient::new(trace.clone());
                let req = Request::builder()
                    .method(Method::POST)
                    .uri(url.as_str())
                    .header("Host", hostname.as_str())
                    .header("Content-Type", "application/dns-message")
                    .header("Accept", "application/dns-message")
                    .header("User-Agent", crate::http::ooni_user_agent())
                    .body(Full::new(Bytes::copy_from_slice(query_wire)))
                    .map_err(|e| OoniError::Unknown(format!("DoH req build: {e}")))?;

                let resp = http.send_http2(tls, req, &addr, "h2", tx_id).await?;
                let (body, _) = crate::http::read_body(resp, 65_535).await;
                Ok(body)
            }
        }
    }

    pub fn engine(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Udp { .. } => "udp",
            Self::Dot { .. } => "dot",
            Self::Doh { .. } => "doh",
        }
    }

    pub fn resolver_address(&self) -> String {
        match self {
            Self::System => String::new(),
            Self::Udp { server } => server.to_string(),
            Self::Dot { server, .. } => server.to_string(),
            Self::Doh { url, .. } => url.clone(),
        }
    }
}

// TracingResolver

/// A DNS resolver that records every lookup **and all transport-layer events**
/// into a [`Trace`].
pub struct TracingResolver {
    transport: DnsTransport,
    trace: Trace,
}

impl TracingResolver {
    pub fn system(trace: Trace) -> Self {
        Self {
            transport: DnsTransport::System,
            trace,
        }
    }

    /// Plain DNS over UDP.
    pub fn udp(server: SocketAddr, trace: Trace) -> Self {
        Self {
            transport: DnsTransport::Udp { server },
            trace,
        }
    }

    /// DNS over TLS (port 853 by convention).
    pub fn dot(server: SocketAddr, hostname: impl Into<String>, trace: Trace) -> Self {
        Self {
            transport: DnsTransport::Dot {
                server,
                hostname: hostname.into(),
            },
            trace,
        }
    }

    /// DNS over HTTPS.
    pub fn doh(
        server: SocketAddr,
        hostname: impl Into<String>,
        url: impl Into<String>,
        trace: Trace,
    ) -> Self {
        Self {
            transport: DnsTransport::Doh {
                server,
                hostname: hostname.into(),
                url: url.into(),
            },
            trace,
        }
    }

    /// Parse a resolver URL and build the appropriate transport.
    ///
    /// Supported schemes: `udp://`, `dot://`, `https://`, `doh://`.
    /// If the host is a hostname (not an IP literal), it is resolved once
    /// using the OS resolver before returning — that resolution is **not** traced.
    pub async fn from_url(url: &str, trace: Trace) -> Result<Self, OoniError> {
        let u = url::Url::parse(url)
            .map_err(|e| OoniError::Unknown(format!("invalid resolver URL: {e}")))?;

        let host = u
            .host_str()
            .ok_or_else(|| OoniError::Unknown("URL has no host".into()))?
            .to_owned();

        let default_port = match u.scheme() {
            "udp" => 53,
            "dot" | "tls" => 853,
            "https" | "doh" => 443,
            other => return Err(OoniError::Unknown(format!("unsupported scheme: {other}"))),
        };
        let port = u.port().unwrap_or(default_port);

        let server = resolve_to_socket_addr(&host, port).await?;

        match u.scheme() {
            "udp" => Ok(Self::udp(server, trace)),
            "dot" | "tls" => Ok(Self::dot(server, &host, trace)),
            "https" | "doh" => Ok(Self::doh(server, &host, url, trace)),
            other => Err(OoniError::Unknown(format!("unsupported scheme: {other}"))),
        }
    }

    /// Resolve `hostname`, issuing both A and AAAA queries.
    /// Records all events into the trace.
    pub async fn lookup_host(&self, hostname: &str) -> Result<Vec<IpAddr>, OoniError> {
        let tx_id = self.trace.next_transaction_id();
        let mut addrs = Vec::new();
        addrs.extend(
            self.do_lookup(hostname, RecordType::A, tx_id)
                .await
                .unwrap_or_default(),
        );
        addrs.extend(
            self.do_lookup(hostname, RecordType::AAAA, tx_id)
                .await
                .unwrap_or_default(),
        );
        if addrs.is_empty() {
            Err(OoniError::DnsNoAnswer)
        } else {
            Ok(addrs)
        }
    }

    /// Issue a single A query.
    pub async fn lookup_a(&self, hostname: &str) -> Result<Vec<IpAddr>, OoniError> {
        let tx_id = self.trace.next_transaction_id();
        self.do_lookup(hostname, RecordType::A, tx_id).await
    }

    /// Issue a single AAAA query.
    pub async fn lookup_aaaa(&self, hostname: &str) -> Result<Vec<IpAddr>, OoniError> {
        let tx_id = self.trace.next_transaction_id();
        self.do_lookup(hostname, RecordType::AAAA, tx_id).await
    }

    async fn do_lookup(
        &self,
        hostname: &str,
        qtype: RecordType,
        tx_id: i64,
    ) -> Result<Vec<IpAddr>, OoniError> {
        let t0 = self.trace.elapsed_secs();

        // Perform the lookup via whichever transport is configured.
        let outcome: Result<(Vec<DnsAnswer>, i64), OoniError> = match &self.transport {
            DnsTransport::System => self.system_lookup(hostname, qtype).await,
            transport => {
                let (_id, wire) = build_query(hostname, qtype)?;
                transport
                    .exchange(&wire, &self.trace, tx_id)
                    .await
                    .and_then(|resp| parse_response(&resp, qtype))
            }
        };

        let t = self.trace.elapsed_secs();
        let qtype_str = match qtype {
            RecordType::A => "A",
            RecordType::AAAA => "AAAA",
            _ => "UNKNOWN",
        };

        let (answers, rcode, ips, failure) = match outcome {
            Ok((ans, rc)) => {
                let ips = answers_to_ips(&ans);
                (ans, Some(rc), ips, None)
            }
            Err(ref e) => (vec![], None, vec![], Some(e.failure().0.clone())),
        };

        // Always record one DnsLookupResult per query, even on failure.
        self.trace.record_dns_lookup(DnsLookupResult {
            answers,
            engine: self.transport.engine().to_owned(),
            failure: failure.clone(),
            hostname: hostname.to_owned(),
            query_type: qtype_str.to_owned(),
            resolver_address: self.transport.resolver_address(),
            resolver_hostname: None,
            resolver_port: None,
            raw_response: None,
            rcode,
            t0,
            t,
            tags: vec![],
            transaction_id: Some(tx_id),
        });

        if failure.is_some() {
            Err(OoniError::DnsNoAnswer)
        } else {
            Ok(ips)
        }
    }

    /// System getaddrinfo via hickory resolver (opaque — no wire events).
    async fn system_lookup(
        &self,
        hostname: &str,
        qtype: RecordType,
    ) -> Result<(Vec<DnsAnswer>, i64), OoniError> {
        use hickory_resolver::{
            config::{ResolverConfig, ResolverOpts},
            TokioAsyncResolver,
        };

        let r = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

        let result: Result<Vec<IpAddr>, _> = if qtype == RecordType::AAAA {
            r.ipv6_lookup(hostname)
                .await
                .map(|l| l.iter().map(|a| IpAddr::V6(*a)).collect())
        } else {
            r.ipv4_lookup(hostname)
                .await
                .map(|l| l.iter().map(|a| IpAddr::V4(*a)).collect())
        };

        match result {
            Ok(ips) => {
                let answers = ips
                    .iter()
                    .map(|ip| match ip {
                        IpAddr::V4(v4) => DnsAnswer {
                            answer_type: "A".into(),
                            ipv4: Some(v4.to_string()),
                            ..Default::default()
                        },
                        IpAddr::V6(v6) => DnsAnswer {
                            answer_type: "AAAA".into(),
                            ipv6: Some(v6.to_string()),
                            ..Default::default()
                        },
                    })
                    .collect();
                Ok((answers, 0))
            }
            Err(e) => Err(classify_hickory(&e)),
        }
    }
}

/// Resolve `host:port` to a `SocketAddr` using the OS resolver.
/// Returns the first address found.
async fn resolve_to_socket_addr(host: &str, port: u16) -> Result<SocketAddr, OoniError> {
    let target = format!("{}:{}", host, port);
    // Try parsing as an IP literal first (no lookup needed).
    if let Ok(addr) = target.parse::<SocketAddr>() {
        return Ok(addr);
    }
    // Fall back to OS resolution.
    tokio::net::lookup_host(&target)
        .await
        .map_err(|e| OoniError::DnsUnexpected(e.to_string()))?
        .next()
        .ok_or(OoniError::DnsNoAnswer)
}

fn classify_hickory(e: &hickory_resolver::error::ResolveError) -> OoniError {
    use hickory_resolver::error::ResolveErrorKind;
    match e.kind() {
        ResolveErrorKind::NoRecordsFound { .. } => OoniError::DnsNoAnswer,
        ResolveErrorKind::Timeout => OoniError::DnsTimeout,
        _ => OoniError::DnsUnexpected(e.to_string()),
    }
}

/// Returns `true` for addresses that should not appear in public DNS.
pub fn is_bogon(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}
