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
