use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::io;

#[cfg(target_os = "ios")]
mod reqwest_impl;
#[cfg(target_os = "ios")]
pub use reqwest_impl::{Client, ClientBuilder};

#[cfg(not(target_os = "ios"))]
mod wreq_impl;
#[cfg(not(target_os = "ios"))]
pub use wreq_impl::{Client, ClientBuilder};

fn b64_encode(b: &[u8]) -> String {
    general_purpose::STANDARD.encode(b)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClientOptions {
    base_url: Option<String>,
    timeout: Option<f32>,
    user_agent: Option<String>,
}

impl ClientOptions {
    pub fn new() -> Self {
        Self {
            base_url: None,
            timeout: None,
            user_agent: None,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidHttpMethod,
    UndetectedCharset,
    Serialization,
    Io(io::Error),
    #[cfg(not(target_os = "ios"))]
    Wreq(Box<wreq::Error>),
    #[cfg(target_os = "ios")]
    Reqwest(Box<reqwest::Error>),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<wreq::Error> for Error {
    fn from(error: wreq::Error) -> Self {
        Self::Wreq(Box::new(error))
    }
}

#[cfg(target_os = "ios")]
impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Reqwest(Box::new(error))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub status_code: u16,
    pub version: String,
    // We place inside of text the headers which we can parse to a string and in bytes those which cannot be parsed as string as a base64 encoding of them.
    pub headers_list_text: Vec<(String, String)>,
    pub headers_list_b64_bytes: Vec<(String, String)>,
    pub body_text: Option<String>,
    pub body_b64_bytes: Option<String>,
}

impl Response {
    pub fn to_json_str(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(|_| Error::Serialization)
    }
}
