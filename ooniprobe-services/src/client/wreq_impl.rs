use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use mime::Mime;
use std::{iter::Product, sync::Arc};
use std::time::Duration;
use tokio::runtime::Runtime;
use wreq::tls::CertStore;
use wreq_util::Emulation;

use super::{b64_encode, ClientOptions, Error, Response};

pub struct Client {
    inner: Arc<ClientRef>,
    rt: Runtime,
}

struct ClientRef {
    http_client: wreq::Client,
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

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub fn execute(&self, request: wreq::Request) -> Result<Response, Error> {
        self.rt.block_on(async {
            let wreq_resp: wreq::Response = self.inner.http_client.execute(request).await?;

            let status_code = wreq_resp.status().as_u16();
            let version = match wreq_resp.version() {
                wreq::Version::HTTP_09 => "HTTP/0.9",
                wreq::Version::HTTP_10 => "HTTP/1.0",
                wreq::Version::HTTP_11 => "HTTP/1.1",
                wreq::Version::HTTP_2 => "HTTP/2.0",
                wreq::Version::HTTP_3 => "HTTP/3.0",
                _ => unreachable!(),
            }
            .to_string();

            let headers = wreq_resp.headers().clone();

            let mut headers_list_text = Vec::new();
            let mut headers_list_b64_bytes = Vec::new();
            for (key, value) in &headers {
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

            let resp_bytes = wreq_resp.bytes().await?;
            let (body_text, body_b64_bytes) = match decode_to_text(&resp_bytes, &headers) {
                Ok(r) => (Some(r), None),
                Err(_) => (None, Some(b64_encode(resp_bytes.as_ref()))),
            };

            Ok(Response {
                status_code,
                version,
                headers_list_text,
                headers_list_b64_bytes,
                body_text,
                body_b64_bytes,
            })
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
                proxy_url: None,
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

        if let Some(proxy_url) = self.client_options.proxy_url {
            let proxy = wreq::Proxy::all(proxy_url)
                .map_err(|e| Error::Wreq(Box::new(e)))?;
        
            client_builder = client_builder.proxy(proxy);
        }

        let http_client = client_builder
            .build()
            .map_err(|e| Error::Wreq(Box::new(e)))?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");

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
