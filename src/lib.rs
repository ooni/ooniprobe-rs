use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};
use url;

pub struct ProbeMeta {
    pub probe_cc: String,
    pub probe_asn: u64,
    pub network_type: String,
    pub software_name: String,
    pub software_version: String,
}

#[derive(Serialize, Deserialize)]
pub struct CheckInRequest {
    pub probe_cc: String,
    pub probe_asn: u64,
    pub probe_network_name: String,
    pub conf: HashMap<String, serde_json::Value>,
    pub tests: HashMap<String, serde_json::Value>,
}

impl CheckInRequest {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

pub struct ProbeServicesConfig {
    pub base_url: url::Url,
}

impl ProbeServicesConfig {
    pub fn default() -> Self {
        Self {
            base_url: url::Url::parse("https://api.ooni.org").unwrap(),
        }
    }
}

pub struct ProbeServicesClient<'a> {
    client: reqwest::Client,
    config: &'a ProbeServicesConfig,
    probe_meta: &'a ProbeMeta,
}

impl<'a> ProbeServicesClient<'a> {
    pub fn new(probe_meta: &'a ProbeMeta, config: &'a ProbeServicesConfig) -> Self {
        Self {
            probe_meta,
            config,
            client: reqwest::Client::new(),
        }
    }

    pub fn config(&mut self, config: &'a ProbeServicesConfig) {
        self.config = config;
    }

    pub fn probe_meta(&mut self, probe_meta: &'a ProbeMeta) {
        self.probe_meta = probe_meta;
    }

    pub async fn get_oonirun_full_descriptor(
        &self,
        oonirun_link_id: &str,
        revision: u32,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let u = self
            .config
            .base_url
            .join(
                format!("/api/v2/oonirun/links/{oonirun_link_id}/full-descriptor/{revision}")
                    .as_str(),
            )
            .unwrap();

        self.client.get(u.as_str()).send().await
    }

    pub async fn get_oonirun_engine_descriptor(
        &self,
        oonirun_link_id: &str,
        revision: u32,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let u = self
            .config
            .base_url
            .join(
                format!("/api/v2/oonirun/links/{oonirun_link_id}/engine-descriptor/{revision}")
                    .as_str(),
            )
            .unwrap();

        self.client.get(u.as_str()).send().await
    }

    pub async fn upload_measurement(
        &self,
        report_id: &str,
        upload_json: &'static str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let u = self
            .config
            .base_url
            .join(format!("/report/{report_id}", report_id = report_id).as_str())
            .unwrap();
        self.client.post(u.as_str()).body(upload_json).send().await
    }

    pub async fn check_in(
        &self,
        conf: HashMap<String, serde_json::Value>,
        tests: HashMap<String, serde_json::Value>,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let req = CheckInRequest {
            probe_cc: self.probe_meta.probe_cc.clone(),
            probe_asn: self.probe_meta.probe_asn,
            probe_network_name: self.probe_meta.network_type.clone(),
            conf,
            tests,
        };
        self.client
            .post(self.config.base_url.clone())
            .json(&req)
            .send()
            .await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_get_oonirun_descriptor() {
        let probe_meta = ProbeMeta {
            probe_cc: "US".to_string(),
            probe_asn: 12345,
            network_type: "wifi".to_string(),
            software_name: "ooniprobe-rs".to_string(),
            software_version: "3.0.0".to_string(),
        };
        let config = ProbeServicesConfig::default();

        let client = ProbeServicesClient::new(&probe_meta, &config);
        let resp = client
            .get_oonirun_full_descriptor("10001", 1)
            .await
            .unwrap();
        assert!(resp.status().is_success());
    }

    #[tokio::test]
    async fn test_upload_measurement() {
        let probe_meta = ProbeMeta {
            probe_cc: "US".to_string(),
            probe_asn: 12345,
            network_type: "wifi".to_string(),
            software_name: "ooniprobe-rs".to_string(),
            software_version: "3.0.0".to_string(),
        };
        let config = ProbeServicesConfig::default();
        let client = ProbeServicesClient::new(&probe_meta, &config);
        let result = client
            .upload_measurement("test_report_id", r#"{"test": "data"}"#)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_in() {
        let probe_meta = ProbeMeta {
            probe_cc: "US".to_string(),
            probe_asn: 12345,
            network_type: "wifi".to_string(),
            software_name: "ooniprobe-rs".to_string(),
            software_version: "3.0.0".to_string(),
        };
        let config = ProbeServicesConfig::default();
        let client = ProbeServicesClient::new(&probe_meta, &config);
        let conf = HashMap::new();
        let tests = HashMap::new();
        let resp = client.check_in(conf, tests).await.unwrap();
        print!("{:?}", resp);
        //assert!(result.is_ok());
    }

    #[test]
    fn test_check_in_request_to_json() {
        let mut conf = HashMap::new();
        conf.insert("key".to_string(), json!("value"));
        let mut tests = HashMap::new();
        tests.insert("test_key".to_string(), json!("test_value"));

        let req = CheckInRequest {
            probe_cc: "US".to_string(),
            probe_asn: 12345,
            probe_network_name: "wifi".to_string(),
            conf,
            tests,
        };

        let json = req.to_json().unwrap();
        assert!(json.contains("\"probe_cc\":\"US\""));
        assert!(json.contains("\"probe_asn\":12345"));
        assert!(json.contains("\"probe_network_name\":\"wifi\""));
        assert!(json.contains("\"key\":\"value\""));
        assert!(json.contains("\"test_key\":\"test_value\""));
    }
}
