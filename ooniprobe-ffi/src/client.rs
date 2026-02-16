use crate::errors::OoniError;

use serde::{Deserialize, Serialize};
use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use ooniprobe_services::client::{Client, Response};

// Must match the UDL dictionary definitions.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: i32,
    pub version: String,
    pub headers_list_text: Vec<Vec<String>>,
    pub headers_list_b64_bytes: Vec<Vec<String>>,
    pub body_text: Option<String>,
    pub body_b64_bytes: Option<String>,
}

pub fn build_client() -> Result<Client, OoniError> {
    Client::builder()
        .build()
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))
}

fn convert_response(resp: Response) -> HttpResponse {
    HttpResponse {
        status_code: resp.status_code as i32,
        version: resp.version,

        headers_list_text: resp
            .headers_list_text
            .into_iter()
            .map(|(k, v)| vec![k, v])
            .collect(),

        headers_list_b64_bytes: resp
            .headers_list_b64_bytes
            .into_iter()
            .map(|(k, v)| vec![k, v])
            .collect(),

        body_text: resp.body_text,
        body_b64_bytes: resp.body_b64_bytes,
    }
}

pub fn client_get(url: String, headers: Vec<KeyValue>, query: Vec<KeyValue>) -> Result<HttpResponse, OoniError> {
    let client = build_client()?;

    let mut header_map = HeaderMap::new();
    for kv in headers {
        let name = HeaderName::from_bytes(kv.key.as_bytes())
            .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

        let value = HeaderValue::from_str(&kv.value)
            .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;
        
        header_map.insert(name, value);
    }

    let request = client
        .request("GET", &url)
        .map(|b| b.headers(header_map).query(&query))
        .and_then(|b| b.build().map_err(Into::into))
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    let response = client
        .execute(request)
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    Ok(convert_response(response))
}

pub fn client_post(url: String, headers: Vec<KeyValue>, payload: String) -> Result<HttpResponse, OoniError> {
    let client = build_client()?;

    let mut header_map = HeaderMap::new();
    for kv in headers {
        let name = HeaderName::from_bytes(kv.key.as_bytes())
            .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

        let value = HeaderValue::from_str(&kv.value)
            .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;
        
        header_map.insert(name, value);
    }

    let request = client
        .request("POST", &url)
        .map(|b| b.headers(header_map).body(payload))
        .and_then(|b| b.build().map_err(Into::into))
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    let response = client
        .execute(request)
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    Ok(convert_response(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_URL: &str = "https://api.dev.ooni.io";

    #[test]
    fn get_manifest_returns_manifest_version_and_public_params() {
        let url = format!("https://ooniprobe.dev.ooni.io/api/v1/manifest");
        let resp = client_get(
            url, 
            vec![], 
            vec![]
        ).expect("GET manifest should succeed");

        assert_eq!(resp.status_code, 200, "incorrect status_code: {:?}", resp);

        let body_text = resp.body_text.as_ref().expect("body_text missing");

        let parsed: serde_json::Value =
            serde_json::from_str(body_text).expect("response body should be valid JSON");

        assert!(
            parsed.get("meta").and_then(|m| m.get("version")).is_some(),
            "meta.version missing: {:?}",
            resp
        );
        assert!(
            parsed.get("manifest").is_some(),
            "manifest missing: {:?}",
            resp
        )
    }

    #[test]
    fn post_check_in_returns_urls() {
        let payload = serde_json::json!({
            "charging": true,
            "on_wifi": true,
            "platform": "android",
            "probe_asn": "AS3320",
            "probe_cc": "DE",
            "run_type": "manual",
            "software_name": "ooniprobe",
            "software_version": "1.0.0",
            "web_connectivity": {
                "category_codes": ["NEWS"]
            }
        })
        .to_string();

        let url = format!("{BASE_URL}/api/v1/check-in");
        let resp = client_post(
            url,
            vec![KeyValue {
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
            }],
            payload,
        )
        .expect("POST check-in should succeed");

        assert_eq!(resp.status_code, 200, "incorrect status_code: {:?}", resp);

        let body_text = resp.body_text.as_ref().expect("body_text missing");

        let parsed: serde_json::Value =
            serde_json::from_str(body_text).expect("response body should be valid JSON");

        assert!(
            parsed.get("tests").is_some(),
            "tests field missing: {}",
            body_text
        );
        assert!(
            parsed["tests"]["web_connectivity"].is_object(),
            "web_connectivity missing: {}",
            body_text
        );
        assert!(
            parsed["tests"]["web_connectivity"]["urls"].is_array(),
            "urls missing: {}",
            body_text
        );
    }
}
