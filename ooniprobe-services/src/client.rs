use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientOption {
    BaseUrl(String),
    Timeout(u64),
    UserAgent(String),
}

#[derive(Debug)]
pub struct Error {}

pub struct Client {
    inner: Arc<ClientRef>,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub async fn get(&self, url: &str) -> Result<bytes::Bytes, rquest::Error> {
        let resp = self.inner.http_client.get(url).send().await?;
        let b = resp.bytes().await?;
        Ok(b)
    }
}

pub struct ClientRef {
    http_client: rquest::Client,
}

#[derive(Debug)]
pub struct ClientBuilder {
    base_url: Option<String>,
    timeout: Option<Duration>,
    user_agent: Option<String>,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            base_url: None,
            timeout: None,
            user_agent: None,
        }
    }

    pub fn set_option(&mut self, option: ClientOption) -> &mut Self {
        match option {
            ClientOption::BaseUrl(url) => {
                self.base_url = Some(url);
            }
            ClientOption::Timeout(seconds) => {
                self.timeout = Some(Duration::from_secs(seconds));
            }
            ClientOption::UserAgent(agent) => {
                self.user_agent = Some(agent);
            }
        }
        self
    }

    pub fn build(self) -> Result<Client, Error> {
        let mut client_builder =
            rquest::Client::builder().impersonate(rquest::Impersonate::Chrome118);

        if let Some(url) = self.base_url {
            client_builder = client_builder.base_url(&url);
        }

        if let Some(timeout) = self.timeout {
            client_builder = client_builder.timeout(timeout);
        }

        if let Some(agent) = self.user_agent {
            client_builder = client_builder.user_agent(&agent);
        }

        let http_client = client_builder.build().unwrap();

        Ok(Client {
            inner: Arc::new(ClientRef { http_client }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_oonirun_descriptor() {
        let client = Client::builder().build().unwrap();
        let resp = client
            .get("/api/v2/oonirun/links/10001/engine-descriptor/1")
            .await
            .unwrap();
        let utf8_string = String::from_utf8(resp.to_vec()).unwrap();
        println!("{}", utf8_string);
    }
}
