use base64::prelude::BASE64_STANDARD;
use base64::{DecodeError, Engine};

pub fn b64_encode(b: &[u8]) -> String {
    BASE64_STANDARD.encode(b)
}

pub fn b64_decode(s: &str) -> Result<Vec<u8>, DecodeError> {
    BASE64_STANDARD.decode(s).map_err(Into::into)
}
