use boring::ssl::{
    CertificateCompressionAlgorithm, CertificateCompressor, SslConnector, SslCurve, SslMethod,
    SslOptions, SslSignatureAlgorithm, SslVersion,
};
use std::io::Write;

struct BrotliCompressor {
    q: u32,
    lgwin: u32,
}

impl Default for BrotliCompressor {
    fn default() -> Self {
        Self { q: 11, lgwin: 32 }
    }
}

impl CertificateCompressor for BrotliCompressor {
    fn algorithm(&self) -> CertificateCompressionAlgorithm {
        CertificateCompressionAlgorithm::BROTLI
    }

    fn can_compress(&self) -> bool {
        true
    }

    fn can_decompress(&self) -> bool {
        true
    }

    fn compress<W>(&self, input: &[u8], output: &mut W) -> std::io::Result<()>
    where
        W: std::io::Write,
    {
        let mut writer = brotli::CompressorWriter::new(output, 1024, self.q, self.lgwin);
        writer.write_all(&input)?;
        Ok(())
    }

    fn decompress<W>(&self, input: &[u8], output: &mut W) -> std::io::Result<()>
    where
        W: std::io::Write,
    {
        brotli::BrotliDecompress(&mut std::io::Cursor::new(input), output)?;
        Ok(())
    }
}

pub fn make_chrome_config() -> boring::ssl::ConnectConfiguration {
    // Setup TLS stack to parrot latest chrome.
    // ja4 hash matches chrome 131.0.6778.70.
    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.clear_options(
        SslOptions::NO_SSLV2 | SslOptions::NO_SSLV3 | SslOptions::NO_TLSV1 | SslOptions::NO_TLSV1_1,
    );
    connector
        .set_cipher_list("ALL:!aPSK:!ECDSA+SHA1:!3DES")
        .expect("failure to set_cipher_list");
    connector.set_grease_enabled(true);
    connector
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .expect("failure to set min proto version");
    connector
        .set_max_proto_version(Some(SslVersion::TLS1_3))
        .expect("failure to set max proto version");
    connector.enable_signed_cert_timestamps();
    connector
        .set_alpn_protos(b"\x02h2\x08http/1.1")
        .expect("failure to set_alpn_protos");
    connector.enable_ocsp_stapling();

    connector
        .add_certificate_compression_algorithm(BrotliCompressor::default())
        .expect("failure to setup brotli compression");

    connector
        .set_verify_algorithm_prefs(&[
            SslSignatureAlgorithm::ECDSA_SECP256R1_SHA256,
            SslSignatureAlgorithm::RSA_PSS_RSAE_SHA256,
            SslSignatureAlgorithm::RSA_PKCS1_SHA256,
            SslSignatureAlgorithm::ECDSA_SECP384R1_SHA384,
            SslSignatureAlgorithm::RSA_PSS_RSAE_SHA384,
            SslSignatureAlgorithm::RSA_PKCS1_SHA384,
            SslSignatureAlgorithm::RSA_PSS_RSAE_SHA512,
            SslSignatureAlgorithm::RSA_PKCS1_SHA512,
        ])
        .expect("failure to set verify algorithms");

    connector
        .set_curves(&[
            SslCurve::X25519_MLKEM768,
            SslCurve::X25519,
            SslCurve::SECP256R1,
            SslCurve::SECP384R1,
        ])
        .expect("failure to set curves");

    let mut config = connector.build().configure().unwrap();
    config
        .add_application_settings(b"h2", b"")
        .expect("failure to add_application_settings");
    config
        .enable_ech_grease()
        .expect("failure to enable ech_grease");
    config
}
