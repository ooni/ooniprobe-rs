//! OONI error types.
//!
//! https://github.com/ooni/spec/blob/master/data-formats/df-007-errors.md

use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use thiserror::Error;

/// Canonical OONI failure string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureString(pub String);

impl FailureString {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FailureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// All errors that `ooniprobe-network` can produce.
#[derive(Debug, Error)]
pub enum OoniError {
    // DNS
    #[error("dns_nxdomain_error")]
    DnsNxDomain,
    #[error("dns_no_answer")]
    DnsNoAnswer,
    #[error("dns_refused_error")]
    DnsRefused,
    #[error("dns_servfail_error")]
    DnsServFail,
    #[error("dns_lookup_timeout_error")]
    DnsTimeout,
    #[error("dns_bogon_error")]
    DnsBogon,
    #[error("dns_temporary_failure")]
    DnsTemporaryFailure,
    #[error("dns_unexpected_failure: {0}")]
    DnsUnexpected(String),

    // TCP
    #[error("connection_refused")]
    ConnectionRefused,
    #[error("connection_reset")]
    ConnectionReset,
    #[error("generic_timeout_error")]
    GenericTimeout,
    #[error("network_unreachable")]
    NetworkUnreachable,
    #[error("host_unreachable")]
    HostUnreachable,
    #[error("connection_already_closed")]
    ConnectionAlreadyClosed,

    // TLS
    #[error("ssl_invalid_certificate")]
    TlsInvalidCertificate,
    #[error("ssl_unknown_authority")]
    TlsUnknownAuthority,
    #[error("ssl_invalid_hostname")]
    TlsInvalidHostname,
    #[error("ssl_handshake_timeout")]
    TlsHandshakeTimeout,
    #[error("ssl_failed_handshake")]
    TlsFailedHandshake,
    #[error("eof_error")]
    Eof,

    // HTTP
    #[error("http_request_failed")]
    HttpRequestFailed,
    #[error("http_invalid_redirect_location_host")]
    HttpInvalidRedirectLocationHost,
    #[error("http_unexpected_redirect_url")]
    HttpUnexpectedRedirectUrl,
    #[error("http_unexpected_status_code")]
    HttpUnexpectedStatusCode,

    // QUIC
    #[error("quic_incompatible_version")]
    QuicIncompatibleVersion,

    // Generic
    #[error("unknown_failure: {0}")]
    Unknown(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl OoniError {
    /// Return the canonical OONI failure string for this error.
    pub fn failure(&self) -> FailureString {
        FailureString(self.to_string())
    }

    /// Wrap an arbitrary `std::io::Error` into the most specific `OoniError`.
    pub fn from_io(e: std::io::Error) -> Self {
        match e.kind() {
            ErrorKind::ConnectionRefused => Self::ConnectionRefused,
            ErrorKind::ConnectionReset => Self::ConnectionReset,
            ErrorKind::TimedOut | ErrorKind::WouldBlock => Self::GenericTimeout,
            ErrorKind::NetworkUnreachable => Self::NetworkUnreachable,
            ErrorKind::HostUnreachable => Self::HostUnreachable,
            ErrorKind::UnexpectedEof => Self::Eof,
            ErrorKind::BrokenPipe => Self::ConnectionAlreadyClosed,
            _ => Self::Unknown(e.to_string()),
        }
    }

    /// Classify a `rustls::Error` into an OONI failure string.
    pub fn from_tls(e: rustls::Error) -> Self {
        match &e {
            rustls::Error::InvalidCertificate(_) => Self::TlsInvalidCertificate,
            rustls::Error::NoCertificatesPresented => Self::TlsUnknownAuthority,
            rustls::Error::General(s) if s.contains("hostname") => Self::TlsInvalidHostname,
            _ => Self::TlsFailedHandshake,
        }
    }
}

pub fn failure_to_option(e: &Option<OoniError>) -> Option<String> {
    e.as_ref().map(|err| err.failure().0)
}
