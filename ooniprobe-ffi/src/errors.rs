use thiserror;

// UniFFI requires errors to derive thiserror::Error. The variant names
// must match the UDL exactly.

#[derive(Debug, thiserror::Error)]
pub enum OoniError {
    #[error("Null or invalid input: {0}")]
    NullOrInvalidInput(String),

    #[error("Base64 decode error: {0}")]
    Base64DecodeError(String),

    #[error("Binary decode error: {0}")]
    BinaryDecodeError(String),

    #[error("HTTP client error: {0}")]
    HttpClientError(String),

    #[error("Request timed out: {0}")]
    TimeoutError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Request error: {0}")]
    RequestError(String),

    #[error("Response error: {0}")]
    ResponseError(String),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid credential: {0}")]
    InvalidCredential(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<base64::DecodeError> for OoniError {
    fn from(e: base64::DecodeError) -> Self {
        OoniError::Base64DecodeError(e.to_string())
    }
}

impl From<bincode::Error> for OoniError {
    fn from(e: bincode::Error) -> Self {
        OoniError::BinaryDecodeError(e.to_string())
    }
}

impl From<ooniprobe_services::client::Error> for OoniError {
    fn from(e: ooniprobe_services::client::Error) -> Self {
        use ooniprobe_services::client::ErrorKind;
        let msg = format!("{:?}", e);
        match e.kind() {
            ErrorKind::Timeout => OoniError::TimeoutError(msg),
            ErrorKind::Connection => OoniError::ConnectionError(msg),
            ErrorKind::Request => OoniError::RequestError(msg),
            ErrorKind::Response => OoniError::ResponseError(msg),
            ErrorKind::Other => OoniError::HttpClientError(msg),
        }
    }
}

impl From<ooniauth_core::errors::CredentialError> for OoniError {
    fn from(e: ooniauth_core::errors::CredentialError) -> Self {
        OoniError::CryptoError(format!("{:?}", e))
    }
}

impl From<cmz::CMZError> for OoniError {
    fn from(e: cmz::CMZError) -> Self {
        OoniError::CryptoError(format!("{:?}", e))
    }
}

impl From<serde_json::Error> for OoniError {
    fn from(e: serde_json::Error) -> Self {
        OoniError::SerializationError(e.to_string())
    }
}

impl From<std::io::Error> for OoniError {
    fn from(e: std::io::Error) -> Self {
        OoniError::Other(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::OoniError;
    use base64::Engine;
    use ooniprobe_services::client::Error as ClientError;

    #[test]
    fn display_includes_variant_prefix() {
        assert_eq!(
            OoniError::TimeoutError("slow".to_string()).to_string(),
            "Request timed out: slow"
        );
        assert_eq!(
            OoniError::InvalidCredential("bad".to_string()).to_string(),
            "Invalid credential: bad"
        );
    }

    #[test]
    fn base64_error_maps_to_base64_decode_error() {
        let e = base64::prelude::BASE64_STANDARD
            .decode("not base64!")
            .unwrap_err();
        assert!(matches!(
            OoniError::from(e),
            OoniError::Base64DecodeError(_)
        ));
    }

    #[test]
    fn bincode_error_maps_to_binary_decode_error() {
        let e = bincode::deserialize::<u64>(&[1]).unwrap_err();
        assert!(matches!(
            OoniError::from(e),
            OoniError::BinaryDecodeError(_)
        ));
    }

    #[test]
    fn client_error_maps_by_kind() {
        assert!(matches!(
            OoniError::from(ClientError::UndetectedCharset),
            OoniError::ResponseError(_)
        ));
        assert!(matches!(
            OoniError::from(ClientError::InvalidHttpMethod),
            OoniError::HttpClientError(_)
        ));
        assert!(matches!(
            OoniError::from(ClientError::Io(std::io::Error::other("boom"))),
            OoniError::HttpClientError(_)
        ));
    }

    #[test]
    fn credential_error_maps_to_crypto_error() {
        let e = ooniauth_core::errors::CredentialError::CredentialExpired;
        assert!(matches!(OoniError::from(e), OoniError::CryptoError(_)));
    }

    #[test]
    fn cmz_error_maps_to_crypto_error() {
        let e = cmz::CMZError::PrivkeyMissing("cred");
        assert!(matches!(OoniError::from(e), OoniError::CryptoError(_)));
    }

    #[test]
    fn serde_json_error_maps_to_serialization_error() {
        let e = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        assert!(matches!(
            OoniError::from(e),
            OoniError::SerializationError(_)
        ));
    }

    #[test]
    fn io_error_maps_to_other() {
        let e = std::io::Error::other("disk on fire");
        match OoniError::from(e) {
            OoniError::Other(msg) => assert_eq!(msg, "disk on fire"),
            other => panic!("expected Other, got {other:?}"),
        }
    }
}
