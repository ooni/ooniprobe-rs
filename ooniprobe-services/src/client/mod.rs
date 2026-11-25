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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_options_new_is_empty() {
        let options = ClientOptions::new();
        assert!(options.base_url.is_none());
        assert!(options.timeout.is_none());
        assert!(options.user_agent.is_none());
        assert!(options.proxy_url.is_none());
    }

    #[test]
    fn client_options_setters_set_and_clear() {
        let mut options = ClientOptions::new();

        options.set_base_url(Some("https://example.org/"));
        options.set_timeout(Some(1.5));
        options.set_user_agent(Some("agent/1.0"));
        options.set_proxy_url(Some("socks5h://127.0.0.1:9050"));
        assert_eq!(options.base_url.as_deref(), Some("https://example.org/"));
        assert_eq!(options.timeout, Some(1.5));
        assert_eq!(options.user_agent.as_deref(), Some("agent/1.0"));
        assert_eq!(
            options.proxy_url.as_deref(),
            Some("socks5h://127.0.0.1:9050")
        );

        options.set_base_url(None);
        options.set_timeout(None);
        options.set_user_agent(None);
        options.set_proxy_url(None);
        assert!(options.base_url.is_none());
        assert!(options.timeout.is_none());
        assert!(options.user_agent.is_none());
        assert!(options.proxy_url.is_none());
    }

    #[test]
    fn error_kind_for_non_http_errors() {
        assert_eq!(Error::UndetectedCharset.kind(), ErrorKind::Response);
        assert_eq!(Error::InvalidHttpMethod.kind(), ErrorKind::Other);
        assert_eq!(Error::Serialization.kind(), ErrorKind::Other);
        assert_eq!(Error::Io(io::Error::other("boom")).kind(), ErrorKind::Other);
    }

    #[test]
    fn io_error_converts_to_error_io() {
        let err: Error = io::Error::new(io::ErrorKind::NotFound, "missing").into();
        match err {
            Error::Io(e) => assert_eq!(e.kind(), io::ErrorKind::NotFound),
            other => panic!("expected Error::Io, got {other:?}"),
        }
    }

    #[test]
    fn b64_encode_uses_standard_alphabet() {
        assert_eq!(b64_encode(b""), "");
        assert_eq!(b64_encode(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn response_to_json_str_round_trips() {
        let resp = Response {
            status_code: 201,
            version: "HTTP/1.1".to_string(),
            headers_list_text: vec![("content-type".to_string(), "text/plain".to_string())],
            headers_list_b64_bytes: vec![("x-bin".to_string(), "Y2Fm6Q==".to_string())],
            body_text: Some("created".to_string()),
            body_b64_bytes: None,
        };

        let json = resp.to_json_str().unwrap();
        let parsed: Response = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.status_code, 201);
        assert_eq!(parsed.version, "HTTP/1.1");
        assert_eq!(parsed.headers_list_text, resp.headers_list_text);
        assert_eq!(parsed.headers_list_b64_bytes, resp.headers_list_b64_bytes);
        assert_eq!(parsed.body_text.as_deref(), Some("created"));
        assert!(parsed.body_b64_bytes.is_none());
    }
}
