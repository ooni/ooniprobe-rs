//! Experiment configuration parser.
//!
//! An [`ExperimentConfig`] is the single type that both the parser and the
//! executor work with.  It has a name (used only for logging and reporting)
//! and an ordered list of [`Step`]s that fully describe what to do.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsTransport {
    System,
    Udp,
    Dot,
    Doh,
}

impl Default for DnsTransport {
    fn default() -> Self {
        Self::System
    }
}

impl fmt::Display for DnsTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => f.write_str("system"),
            Self::Udp => f.write_str("udp"),
            Self::Dot => f.write_str("dot"),
            Self::Doh => f.write_str("doh"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DnsStep {
    pub hostname: String,

    #[serde(default)]
    pub transport: DnsTransport,

    /// Required for Udp and Dot transports (`"ip:port"`).
    pub server: Option<String>,

    /// Required for Doh transport
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TcpStep {
    pub port: u16,

    /// When set, connect to this address directly (bypasses DNS chaining).
    /// Accepts `"ip"` or `"ip:port"`
    pub address: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsStep {
    /// Server Name Indication sent in the ClientHello.
    /// When absent, falls back to the hostname from the most recent `dns` step.
    pub sni: Option<String>,

    /// Skip certificate verification.  Useful for censorship measurement where
    /// the injected certificate wouldn't pass validation.
    #[serde(default)]
    pub skip_verify: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpVersion {
    H1,
    H2,
    /// Negotiate via ALPN (use h2 if available, else h1).
    Auto,
}

impl Default for HttpVersion {
    fn default() -> Self {
        Self::Auto
    }
}

fn default_http_method() -> String {
    "GET".into()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpStep {
    pub url: String,

    /// HTTP method
    #[serde(default = "default_http_method")]
    pub method: String,

    /// HTTP version to use
    #[serde(default)]
    pub version: HttpVersion,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Step {
    Dns(DnsStep),
    Tcp(TcpStep),
    Tls(TlsStep),
    Http(HttpStep),
}

impl Step {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Dns(_) => "dns",
            Self::Tcp(_) => "tcp",
            Self::Tls(_) => "tls",
            Self::Http(_) => "http",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("validation error: {0}")]
    Validation(String),
}

/// Assembles a complete OONI [`Measurement`] from an experiment run.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProbeConfig {
    pub probe_asn: String,
    pub probe_cc: String,
    pub probe_ip: String,
    pub probe_network_name: String,

    pub software_name: String,
    pub software_version: String,
}

/// A fully-parsed, validated experiment description.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExperimentConfig {
    #[serde(rename = "experiment")]
    pub name: String,
    pub input: Option<String>,
    pub probe_config: ProbeConfig, 
    pub steps: Vec<Step>,
}

impl ExperimentConfig {
    /// Parse from a JSON string and validate step dependencies.
    pub fn from_json(input: &str) -> Result<Self, ParseError> {
        let cfg: Self = serde_json::from_str(input)?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ParseError> {
        if self.steps.is_empty() {
            return Err(ParseError::Validation("steps list is empty".into()));
        }

        let mut has_ips = false;
        let mut has_tcp = false;
        let mut has_tls = false;
        // Remember the last DNS hostname so TLS steps can fall back to it.
        let mut last_dns_hostname: Option<&str> = None;

        for (i, step) in self.steps.iter().enumerate() {
            match step {
                Step::Dns(s) => {
                    if s.transport != DnsTransport::System && s.server.is_none() {
                        return Err(ParseError::Validation(format!(
                            "step {i} (dns): transport '{}' requires a 'server' field",
                            s.transport
                        )));
                    }
                    if s.transport == DnsTransport::Doh && s.url.is_none() {
                        return Err(ParseError::Validation(format!(
                            "step {i} (dns/doh): 'url' is required for DoH"
                        )));
                    }
                    has_ips = true;
                    has_tcp = false;
                    has_tls = false;
                    last_dns_hostname = Some(&s.hostname);
                }

                Step::Tcp(s) => {
                    if s.address.is_none() && !has_ips {
                        return Err(ParseError::Validation(format!(
                            "step {i} (tcp): no 'address' and no preceding dns step to supply addresses"
                        )));
                    }
                    has_ips = false;
                    has_tcp = true;
                }

                Step::Tls(s) => {
                    if !has_tcp {
                        return Err(ParseError::Validation(format!(
                            "step {i} (tls): no preceding tcp step to supply an open stream"
                        )));
                    }
                    // SNI must be resolvable — either explicit or from a prior dns step.
                    if s.sni.is_none() && last_dns_hostname.is_none() {
                        return Err(ParseError::Validation(format!(
                            "step {i} (tls): no 'sni' and no preceding dns step to derive SNI from"
                        )));
                    }
                    has_tcp = false;
                    has_tls = true;
                }

                Step::Http(_) => {
                    if !has_tcp && !has_tls {
                        return Err(ParseError::Validation(format!(
                            "step {i} (http): requires an open stream from a preceding tcp or tls step"
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}
