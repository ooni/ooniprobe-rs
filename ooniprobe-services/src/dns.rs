pub use hickory_resolver::config::LookupIpStrategy;
use hickory_resolver::config::{
    NameServerConfig, NameServerConfigGroup, Protocol, ResolverConfig, ResolverOpts,
};
use hickory_resolver::{lookup_ip::LookupIpIntoIter, TokioAsyncResolver};
use log::warn;
use rquest::dns::{Addrs, Name, Resolve, Resolving};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

/// Wrapper around an `AsyncResolver`, which implements the `Resolve` trait.
#[derive(Debug, Clone)]
pub struct DoHDnsResolver {
    /// Since we might not have been called in the context of a
    /// Tokio Runtime in initialization, so we must delay the actual
    /// construction of the resolver.
    state: Arc<TokioAsyncResolver>,
}

pub struct DohConfig {
    ip: IpAddr,
    port: u16,
    dns_name: String,
}

impl DoHDnsResolver {
    pub fn default() -> rquest::Result<Self> {
        let config = match rand::random::<u8>() % 3 {
            0 => ResolverConfig::google_https(),
            1 => ResolverConfig::cloudflare_https(),
            _ => ResolverConfig::quad9_https(),
        };

        warn!("using {:?}", config.name_servers());

        let mut opts = ResolverOpts::default();
        opts.try_tcp_on_error = true;

        Ok(Self {
            state: Arc::new(TokioAsyncResolver::tokio(config, opts)),
        })
    }

    pub fn new<S: Into<Option<LookupIpStrategy>>>(
        doh_config: Vec<DohConfig>,
        strategy: S,
    ) -> rquest::Result<Self> {
        let mut config = ResolverConfig::default();

        for c in doh_config {
            let ns = NameServerConfig {
                socket_addr: SocketAddr::new(c.ip, c.port),
                protocol: Protocol::Https,
                tls_dns_name: Some(c.dns_name.clone()),
                trust_negative_responses: false,
                tls_config: None,
                bind_addr: None,
            };
            config.add_name_server(ns)
        }
        let mut opts = ResolverOpts::default();
        opts.try_tcp_on_error = true;
        opts.ip_strategy = strategy.into().unwrap_or(LookupIpStrategy::Ipv4AndIpv6);

        Ok(Self {
            state: Arc::new(TokioAsyncResolver::tokio(config, opts)),
        })
    }
}

struct SocketAddrs {
    iter: LookupIpIntoIter,
}

impl Resolve for DoHDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolver = self.clone();
        Box::pin(async move {
            let lookup = resolver.state.lookup_ip(name.as_str()).await?;
            let addrs: Addrs = Box::new(SocketAddrs {
                iter: lookup.into_iter(),
            });
            Ok(addrs)
        })
    }
}

impl Iterator for SocketAddrs {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ip_addr| SocketAddr::new(ip_addr, 0))
    }
}
