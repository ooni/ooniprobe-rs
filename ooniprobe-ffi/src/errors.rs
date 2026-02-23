use thiserror;

// UniFFI requires errors to derive thiserror::Error. The variant names
// must match the UDL exactly.

#[derive(Debug, thiserror::Error)]
pub enum OoniError {
    #[error("Null or invalid input: {0}")]
    NullOrInvalidInput(String),

    #[error("Base64 decode error: {0}")]
    Base64DecodeError(String),

    #[error("Bincode decode error: {0}")]
    BincodeDecodeError(String),

    #[error("HTTP client error: {0}")]
    HttpClientError(String),

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
        OoniError::BincodeDecodeError(e.to_string())
    }
}

impl From<ooniprobe_services::client::Error> for OoniError {
    fn from(e: ooniprobe_services::client::Error) -> Self {
        OoniError::HttpClientError(format!("{:?}", e))
    }
}

impl From<rquest::Error> for OoniError {
    fn from(e: rquest::Error) -> Self {
        OoniError::HttpClientError(e.to_string())
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
