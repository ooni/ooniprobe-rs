use base64::{engine::general_purpose, Engine as _};
use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use mime::Mime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use wreq::tls::CertStore;
use wreq_util::Emulation;

use std::io;
use tokio::runtime::Runtime;

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
    Wreq(Box<wreq::Error>),
    Serialization,
    Io(io::Error),
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
    fn from_request(req: &wreq::Response) -> Self {
        let version = match req.version() {
            wreq::Version::HTTP_09 => "HTTP/0.9",
            wreq::Version::HTTP_10 => "HTTP/1.0",
            wreq::Version::HTTP_11 => "HTTP/1.1",
            wreq::Version::HTTP_2 => "HTTP/2.0",
            wreq::Version::HTTP_3 => "HTTP/3.0",
            _ => unreachable!(),
        };

        let mut headers_list_text = Vec::new();
        let mut headers_list_b64_bytes = Vec::new();

        for (key, value) in req.headers() {
            let header_name = key.to_string();
            match value.to_str() {
                Ok(text_value) => {
                    headers_list_text.push((header_name, text_value.to_string()));
                }
                Err(_) => {
                    headers_list_b64_bytes.push((header_name, b64_encode(value.as_bytes())));
                }
            }
        }
        Self {
            status_code: req.status().as_u16(),
            version: version.to_string(),
            headers_list_text,
            headers_list_b64_bytes,
            body_text: None,
            body_b64_bytes: None,
        }
    }

    pub fn to_json_str(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(|_| Error::Serialization)
    }
}

fn decode_to_text(bytes: &Bytes, headers: &wreq::header::HeaderMap) -> Result<String, Error> {
    let content_type = headers
        .get(wreq::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Mime>().ok());
    let encoding_name = content_type
        .as_ref()
        .and_then(|mime| mime.get_param("charset").map(|charset| charset.as_str()))
        .unwrap_or("utf-8");
    let encoding = Encoding::for_label(encoding_name.as_bytes()).unwrap_or(UTF_8);

    let (text, _, malformed) = encoding.decode(bytes);
    if malformed {
        return Err(Error::UndetectedCharset);
    }
    Ok(text.into_owned())
}

pub struct Client {
    inner: Arc<ClientRef>,
    rt: Runtime,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub fn execute(&self, request: wreq::Request) -> Result<Response, Error> {
        self.rt.block_on(async {
            let wreq_resp: wreq::Response = self.inner.http_client.execute(request).await?;
            let headers = wreq_resp.headers().clone();
            let mut response = Response::from_request(&wreq_resp);
            let resp_bytes = wreq_resp.bytes().await?;
            match decode_to_text(&resp_bytes, &headers) {
                Ok(r) => response.body_text = Some(r),
                Err(_) => response.body_b64_bytes = Some(b64_encode(resp_bytes.as_ref())),
            };
            Ok(response)
        })
    }

    pub fn request(&self, method: &str, url: &str) -> Result<wreq::RequestBuilder, Error> {
        let m = match method.to_uppercase().as_str() {
            "GET" => http::Method::GET,
            "POST" => http::Method::POST,
            "PUT" => http::Method::PUT,
            "PATCH" => http::Method::PATCH,
            "OPTIONS" => http::Method::OPTIONS,
            _ => return Err(Error::InvalidHttpMethod),
        };
        Ok(self.inner.http_client.request(m, url))
    }
}

pub struct ClientRef {
    http_client: wreq::Client,
}

#[derive(Debug, Clone)]
pub struct ClientBuilder {
    client_options: ClientOptions,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            client_options: ClientOptions {
                base_url: Some("https://api.ooni.org/".to_string()),
                timeout: Some(10.0),
                user_agent: Some("ooniprobe".to_string()),
            },
        }
    }

    pub fn set_options(mut self, options: ClientOptions) -> Self {
        self.client_options = options;
        self
    }

    pub fn build(self) -> Result<Client, Error> {
        let mut client_builder = wreq::Client::builder()
            .cert_store(CertStore::from_der_certs(
                webpki_root_certs::TLS_SERVER_ROOT_CERTS,
            )?)
            .emulation(Emulation::Chrome118);

        if let Some(timeout) = self.client_options.timeout {
            client_builder = client_builder.timeout(Duration::from_secs_f32(timeout));
        }

        if let Some(agent) = self.client_options.user_agent {
            client_builder = client_builder.user_agent(&agent);
        }

        let http_client = client_builder.build().expect("failed to build http_client");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to tokio build runtime");

        Ok(Client {
            inner: Arc::new(ClientRef { http_client }),
            rt,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_oonirun_descriptor() {
        let client = Client::builder().build().unwrap();
        let request = client
            .request(
                "GET",
                "https://api.ooni.org/api/v2/oonirun/links/10001/engine-descriptor/1",
            )
            .expect("failed to build request")
            .build()
            .unwrap();
        let resp = client.execute(request).unwrap();
        let resp_json = serde_json::to_string(&resp).unwrap();
        println!("{}", resp_json);
    }

    #[test]
    fn test_binary_data() {
        let mut client_options = ClientOptions::new();
        client_options.timeout = Some(5.0);
        let client = Client::builder()
            .set_options(client_options)
            .build()
            .unwrap();
        let request = client
            .request("GET", "https://httpbin.org/stream-bytes/100")
            .expect("failed to build request")
            .build()
            .unwrap();
        let resp = client.execute(request).unwrap();
        assert_eq!(resp.body_b64_bytes.is_some(), true);
        assert_eq!(resp.body_text.is_none(), true);
        let resp_json = serde_json::to_string(&resp).unwrap();
        println!("{}", resp_json);
    }
}
