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
    proxy_url: Option<String>,
}

impl ClientOptions {
    pub fn new() -> Self {
        Self {
            base_url: None,
            timeout: None,
            user_agent: None,
            proxy_url: None,
        }
    }

    pub fn set_base_url(&mut self, base_url: Option<&str>) {
        self.base_url = base_url.map(String::from);
    }

    pub fn set_timeout(&mut self, timeout: Option<f32>) {
        self.timeout = timeout;
    }

    pub fn set_user_agent(&mut self, user_agent: Option<&str>) {
        self.user_agent = user_agent.map(String::from);
    }

    pub fn set_proxy_url(&mut self, proxy_url: Option<&str>) {
        self.proxy_url = proxy_url.map(String::from);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Timeout,
    Connection,
    Request,
    Response,
    Other,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::UndetectedCharset => ErrorKind::Response,
            Error::InvalidHttpMethod | Error::Serialization | Error::Io(_) => ErrorKind::Other,
            #[cfg(not(target_os = "ios"))]
            Error::Wreq(e) => classify_wreq(e),
            #[cfg(target_os = "ios")]
            Error::Reqwest(e) => classify_reqwest(e),
        }
    }
}

#[cfg(not(target_os = "ios"))]
fn classify_wreq(e: &wreq::Error) -> ErrorKind {
    if e.is_timeout() {
        ErrorKind::Timeout
    } else if e.is_connect() || e.is_proxy_connect() || e.is_connection_reset() {
        ErrorKind::Connection
    } else if e.is_body() || e.is_decode() {
        ErrorKind::Response
    } else if e.is_request() {
        ErrorKind::Request
    } else {
        ErrorKind::Other
    }
}

#[cfg(target_os = "ios")]
fn classify_reqwest(e: &reqwest::Error) -> ErrorKind {
    if e.is_timeout() {
        ErrorKind::Timeout
    } else if e.is_connect() {
        ErrorKind::Connection
    } else if e.is_body() || e.is_decode() {
        ErrorKind::Response
    } else if e.is_request() {
        ErrorKind::Request
    } else {
        ErrorKind::Other
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(not(target_os = "ios"))]
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
