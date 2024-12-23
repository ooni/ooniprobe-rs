use std::{str::FromStr, sync::Arc};

use tokio::runtime::Runtime;

use crate::http2_client;

pub struct ConnectConfig {
    host: Arc<str>,
    port: u32,
}

pub struct ProbeServicesClient {
    connect_config: ConnectConfig,
}

impl ProbeServicesClient {
    pub fn new() -> Self {
        Self {
            connect_config: ConnectConfig {
                host: "api.ooni.org".into(),
                port: 443,
            },
        }
    }

    pub fn request(&self, path_and_query: String, method: String) -> Option<bytes::Bytes> {
        let rt = Runtime::new().unwrap();

        let p = http::uri::PathAndQuery::from_str(path_and_query.clone().as_str()).unwrap();
        let res = rt
            .block_on(http2_client::connect_and_send_request(
                self.connect_config.host.clone(),
                self.connect_config.port,
                p,
                http::Method::from_bytes(method.as_bytes()).unwrap(),
                http::HeaderMap::new(),
                bytes::Bytes::new(),
            ))
            .unwrap();
        Some(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_oonirun_descriptor() {
        let req_path = String::from("/api/v2/oonirun/links/10001/engine-descriptor/1");

        let client = ProbeServicesClient::new();
        let resp = client.request(req_path, String::from("GET"));
        if let Some(bytes) = resp {
            let utf8_string = String::from_utf8(bytes.to_vec()).unwrap();
            println!("{}", utf8_string);
        } else {
            println!("Request failed");
        }
    }
}
