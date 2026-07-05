use bytes::Bytes;
use encoding_rs::{Encoding, UTF_8};
use mime::Mime;
use std::time::Duration;
use tokio::runtime::Runtime;

use super::{b64_encode, ClientOptions, Error, Response};

pub struct Client {
    http_client: reqwest::Client,
    rt: Runtime,
}

fn decode_to_text(bytes: &Bytes, headers: &reqwest::header::HeaderMap) -> Result<String, Error> {
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
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

    pub fn execute(&self, request: reqwest::Request) -> Result<Response, Error> {
        self.rt.block_on(async {
            let resp: reqwest::Response = self.http_client.execute(request).await?;

            let status_code = resp.status().as_u16();
            let version = match resp.version() {
                reqwest::Version::HTTP_09 => "HTTP/0.9",
                reqwest::Version::HTTP_10 => "HTTP/1.0",
                reqwest::Version::HTTP_11 => "HTTP/1.1",
                reqwest::Version::HTTP_2 => "HTTP/2.0",
                reqwest::Version::HTTP_3 => "HTTP/3.0",
                _ => unreachable!(),
            }
            .to_string();

            let headers = resp.headers().clone();

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

            let resp_bytes = resp.bytes().await?;
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

    pub fn request(&self, method: &str, url: &str) -> Result<reqwest::RequestBuilder, Error> {
        let m = match method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            "OPTIONS" => reqwest::Method::OPTIONS,
            _ => return Err(Error::InvalidHttpMethod),
        };
        Ok(self.http_client.request(m, url))
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
                timeout: Some(20.0),
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
        let mut client_builder = reqwest::Client::builder().use_rustls_tls();

        if let Some(timeout) = self.client_options.timeout {
            client_builder = client_builder.timeout(Duration::from_secs_f32(timeout));
        }

        if let Some(agent) = self.client_options.user_agent {
            client_builder = client_builder.user_agent(agent);
        }

        if let Some(proxy_url) = self.client_options.proxy_url {
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| Error::Reqwest(Box::new(e)))?;

            client_builder = client_builder.proxy(proxy);
        }

        let http_client = client_builder
            .build()
            .map_err(|e| Error::Reqwest(Box::new(e)))?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");

        Ok(Client { http_client, rt })
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
