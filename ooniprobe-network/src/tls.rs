//! TLS handshake with tracing.
//!
//! [`TracingTlsHandshaker`] performs a TLS 1.2/1.3 handshake over any
//! `AsyncRead + AsyncWrite` stream and emits a [`TlsHandshakeResult`]

use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{client::TlsStream, TlsConnector};

use crate::{
    archival::{BinaryData, TlsHandshakeResult, TlsNetwork},
    errors::OoniError,
    trace::Trace,
};

/// Build a `ClientConfig` with the system root store.
pub fn system_tls_config() -> Result<Arc<ClientConfig>, OoniError> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

/// Build a `ClientConfig` that skips certificate verification.
pub fn insecure_tls_config() -> Arc<ClientConfig> {
    #[derive(Debug)]
    struct NoVerifier;

    impl rustls::client::danger::ServerCertVerifier for NoVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self, _msg: &[u8], _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self, _msg: &[u8], _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    Arc::new(
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth(),
    )
}

// TracingTlsHandshaker

/// Performs TLS handshakes and records results in a [`Trace`].
pub struct TracingTlsHandshaker {
    config: Arc<ClientConfig>,
    trace: Trace,
    no_tls_verify: bool,
}

impl TracingTlsHandshaker {
    /// Create a handshaker using system CA roots.
    pub fn new(trace: Trace) -> Result<Self, OoniError> {
        Ok(Self {
            config: system_tls_config()?,
            trace,
            no_tls_verify: false,
        })
    }

    /// Create a handshaker that skips certificate validation.
    pub fn insecure(trace: Trace) -> Self {
        Self {
            config: insecure_tls_config(),
            trace,
            no_tls_verify: true,
        }
    }

    /// Create a handshaker with a custom [`ClientConfig`].
    pub fn with_config(config: Arc<ClientConfig>, trace: Trace) -> Self {
        Self { config, trace, no_tls_verify: false }
    }

    /// Perform a TLS handshake over `stream`.
    pub async fn handshake<S>(
        &self,
        stream: S,
        server_name: &str,
        address: &str,
        tx_id: i64,
    ) -> Result<TlsStream<S>, OoniError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let connector = TlsConnector::from(self.config.clone());
        let sni = rustls::pki_types::ServerName::try_from(server_name.to_owned())
            .map_err(|e| OoniError::Unknown(format!("invalid SNI: {e}")))?;

        let t0 = self.trace.elapsed_secs();
        let result = connector.connect(sni, stream).await;
        let t = self.trace.elapsed_secs();

        match result {
            Ok(tls_stream) => {
                let (_, session) = tls_stream.get_ref();
                let cipher = session
                    .negotiated_cipher_suite()
                    .map(|cs| format!("{:?}", cs.suite()))
                    .unwrap_or_default();
                let alpn = session
                    .alpn_protocol()
                    .map(|a| String::from_utf8_lossy(a).into_owned())
                    .unwrap_or_default();
                let tls_version = session
                    .protocol_version()
                    .map(|v| format!("{:?}", v))
                    .unwrap_or_default();
                let peer_certs = session
                    .peer_certificates()
                    .unwrap_or_default()
                    .iter()
                    .map(|c| BinaryData(c.as_ref().to_vec()))
                    .collect();

                self.trace.record_tls_handshake(TlsHandshakeResult {
                    network: Some(TlsNetwork::Tcp),
                    address: address.to_owned(),
                    cipher_suite: cipher,
                    failure: None,
                    negotiated_protocol: alpn,
                    no_tls_verify: self.no_tls_verify,
                    peer_certificates: peer_certs,
                    server_name: Some(server_name.to_owned()),
                    t0,
                    t,
                    tags: None,
                    tls_version,
                    transaction_id: Some(tx_id),
                    outer_server_name: None,
                    ech_config: None,
                    so_error: None,
                });

                Ok(tls_stream)
            }
            Err(e) => {
                // Try to classify as rustls vs I/O error.
                let ooni_err = if let Some(tls_err) = e.get_ref()
                    .and_then(|e| e.downcast_ref::<rustls::Error>())
                {
                    OoniError::from_tls(tls_err.clone())
                } else {
                    OoniError::from_io(e)
                };

                self.trace.record_tls_handshake(TlsHandshakeResult {
                    network: Some(TlsNetwork::Tcp),
                    address: address.to_owned(),
                    cipher_suite: String::new(),
                    failure: Some(ooni_err.failure().0.clone()),
                    negotiated_protocol: String::new(),
                    no_tls_verify: self.no_tls_verify,
                    peer_certificates: vec![],
                    server_name: Some(server_name.to_owned()),
                    t0,
                    t,
                    tags: None,
                    tls_version: String::new(),
                    transaction_id: Some(tx_id),
                    outer_server_name: None,
                    ech_config: None,
                    so_error: None,
                });

                Err(ooni_err)
            }
        }
    }
}
