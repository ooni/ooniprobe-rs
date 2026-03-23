use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bincode;
use cmz::*;
use curve25519_dalek::{ristretto::RistrettoPoint as G, RistrettoPoint};
use ooniauth_core::registration::UserAuthCredential;
use rand;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};

use crate::client::build_client;
use crate::errors::OoniError;
use crate::HttpResponse;
use ooniauth_core::PublicParameters;

#[derive(Debug)]
pub struct ProbeIDResult {
    pub probe_id: String,
}

#[derive(Debug)]
pub struct CredentialResult {
    pub credential: Option<String>,
    pub response: HttpResponse,
}

#[derive(Serialize, Deserialize)]
struct RegistrationPayload {
    manifest_version: String,
    credential_sign_request: String,
}

#[derive(Serialize, Deserialize)]
struct RegistrationResponse {
    credential_sign_response: String,
    emission_day: i32,
}

#[derive(Serialize, Deserialize)]
struct SubmitMeasurementPayload {
    format: String,
    content: serde_json::Value,
    nym: String,
    zkp_request: String,
    probe_age_range: (u32, u32),
    probe_msm_range: (u32, u32),
    manifest_version: String,
}

#[derive(Serialize, Deserialize)]
struct SubmitMeasurementResponse {
    measurement_uid: Option<String>,
    is_verified: bool,
    submit_response: Option<String>,
}

fn b64_encode(b: &[u8]) -> String {
    BASE64_STANDARD.encode(b)
}

fn b64_decode(s: &str) -> Result<Vec<u8>, OoniError> {
    BASE64_STANDARD.decode(s).map_err(Into::into)
}

fn decode_public_params(public_params: &str) -> Result<PublicParameters, OoniError> {
    cmz_group_init(G::hash_from_bytes::<Sha512>(b"CMZ Generator A"));
    let raw = b64_decode(public_params)?;
    bincode::deserialize(&raw).map_err(Into::into)
}

fn decode_credential(credential: &str) -> Result<UserAuthCredential, OoniError> {
    let cred_bytes = b64_decode(credential)?;
    bincode::deserialize(&cred_bytes).map_err(Into::into)
}

fn digest_point(point: RistrettoPoint) -> [u8; 32] {
    let digest = Sha256::digest(point.compress().as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn get_probe_id(
    credential_b64: String,
    probe_asn: String,
    probe_cc: String,
) -> Result<ProbeIDResult, OoniError> {
    let credential = decode_credential(&credential_b64)?;
    let domain_str = format!("ooni.org/{}/{}", probe_cc, probe_asn);
    let domain = G::hash_from_bytes::<Sha512>(domain_str.as_bytes());

    let nym = credential
        .nym_id
        .ok_or(OoniError::InvalidCredential(String::from(
            "invalid credential",
        )))?
        * domain;
    let raw_id = digest_point(nym);

    Ok(ProbeIDResult {
        probe_id: hex::encode(raw_id),
    })
}

pub fn userauth_register(
    url: String,
    public_params: String,
    manifest_version: String,
) -> Result<CredentialResult, OoniError> {
    // initialize user state with public params
    let pp = decode_public_params(&public_params)?;
    let mut user_state = ooniauth_core::UserState::new(pp);

    // prepare registration request
    let mut rng = rand::thread_rng();
    let (reg_request, reg_state) = user_state.request(&mut rng)?;
    let raw_bytes = reg_request.as_bytes();
    let request_payload = b64_encode(&raw_bytes);

    // prepare payload for POST to register endpoint
    let payload = RegistrationPayload {
        manifest_version: manifest_version,
        credential_sign_request: request_payload,
    };
    let json_payload = serde_json::to_string(&payload)?;

    // make the API call
    let client = build_client()?;
    let request = client
        .request("POST", &url)
        .map(|b| b.body(json_payload))
        .and_then(|b| b.build().map_err(Into::into))?;

    let response: HttpResponse = client.execute(request).map(Into::into)?;

    // return early in case of failure
    if response.status_code < 200 || response.status_code >= 300 {
        return Ok(CredentialResult {
            credential: None,
            response: response,
        });
    }

    let body_text = response.body_text.as_ref().ok_or_else(|| {
        OoniError::HttpClientError(String::from("Empty response body from server"))
    })?;

    let resp: RegistrationResponse = serde_json::from_str(body_text)?;

    let reply_bytes = b64_decode(&resp.credential_sign_response)?;
    let reply = bincode::deserialize::<ooniauth_core::registration::open_registration::Reply>(&reply_bytes)?;

    // handle API response in user state
    user_state.handle_response(reg_state, reply)?;

    let credential = user_state
        .get_credential()
        .ok_or(OoniError::InvalidCredential(String::from(
            "invalid credential",
        )))?;

    // serialize the full credential object so the caller can store it
    let cred_bytes = bincode::serialize(credential)?;

    Ok(CredentialResult {
        credential: Some(b64_encode(&cred_bytes)),
        response,
    })
}

pub fn userauth_submit(
    url: String,
    credential: String,
    public_params: String,
    content: String,
    probe_cc: String,
    probe_asn: String,
    manifest_version: String,
    age: u32,
) -> Result<CredentialResult, OoniError> {
    // initialize the user state with public params
    let pp = decode_public_params(&public_params)?;
    let mut user_state = ooniauth_core::UserState::new(pp);

    let credential = decode_credential(&credential)?;
    user_state.set_credential(credential.clone());

    let age_range = age.saturating_sub(30)..age.saturating_add(1);
    let measurement_count_range = 0..3000;

    // prepare submission request
    let mut rng = rand::thread_rng();
    let ((submit_request, submit_state), probe_id) = user_state.submit_request(
        &mut rng,
        probe_cc.clone(),
        probe_asn.clone(),
        age_range.clone(),
        measurement_count_range.clone(),
    )?;

    // prepare payload for POST to submit endpoint
    let measurement_content: serde_json::Value = serde_json::from_str(&content)?;
    let request_bytes = submit_request.as_bytes();
    let submit_payload = SubmitMeasurementPayload {
        format: "json".to_string(),
        content: measurement_content,
        nym: b64_encode(&probe_id),
        zkp_request: b64_encode(&request_bytes),
        probe_age_range: (age_range.start, age_range.end),
        probe_msm_range: (measurement_count_range.start, measurement_count_range.end),
        manifest_version,
    };

    let json_payload = serde_json::to_string(&submit_payload)?;

    // make the API call
    let client = build_client()?;
    let request = client
        .request("POST", &url)
        .map(|b| b.body(json_payload))
        .and_then(|b| b.build().map_err(Into::into))?;

    let response: HttpResponse = client.execute(request).map(Into::into)?;

    // return early in case of failure
    if response.status_code < 200 || response.status_code >= 300 {
        return Ok(CredentialResult {
            credential: None,
            response: response,
        });
    }

    let body_text = response.body_text.as_ref().ok_or_else(|| {
        OoniError::HttpClientError(String::from("Empty response body from server"))
    })?;

    let resp: SubmitMeasurementResponse = serde_json::from_str(&body_text)?;
    let Some(submit_b64) = resp.submit_response else {
        return Ok(CredentialResult {
            credential: None,
            response: response,
        });
    };

    let reply_bytes = b64_decode(&submit_b64)?;
    let reply = bincode::deserialize::<ooniauth_core::submit::submit::Reply>(&reply_bytes)?;

    // handle API response in user state
    user_state.handle_submit_response(submit_state, reply)?;

    let credential = user_state
        .get_credential()
        .ok_or(OoniError::InvalidCredential(String::from(
            "invalid credential",
        )))?;

    // serialize the full credential object so the caller can store it
    let cred_bytes = bincode::serialize(credential)?;

    Ok(CredentialResult {
        credential: Some(b64_encode(&cred_bytes)),
        response,
    })
}

#[cfg(test)]
mod tests {
    use crate::client::{client_post, KeyValue};
    use crate::get_probe_id;
    use crate::userauth::{userauth_register, userauth_submit};

    const BASE_URL: &str = "https://api.dev.ooni.io";

    #[test]
    fn userauth_register_works_with_public_params() {
        let url = format!("{BASE_URL}/api/v1/sign_credential");

        let public_params = "AYAWz7F8oKPtK+mHf/RJw2kBcQ+r5gT81HHsiM+3ZEJQAUyrROFBhwftdH6IJV69nYHy3bRuHmc27BmsJx966p80AwAAAAAAAABc8OIsTyiGCSjp3xT0rvevfKX6Qv2rg//nn9RcjBsPKzoQNZdMymjjkiOAYUxg9WgfCxN/lJvn6hcLt4a+MrJXcgXPtzlDa8cvtauhi6Um4THT+h4L/0zW3AxfmZTVw1Q=";
        let manifest_version = "DJF88g0blInW8uw4zodNNZkdOd3UcXAx";

        let result =
            userauth_register(url, public_params.to_string(), manifest_version.to_string())
                .expect("The FFI call itself should not throw an OoniError");

        assert_eq!(
            result.response.status_code, 200,
            "Server should return 200 OK. Body: {:?}",
            result.response.body_text
        );

        let credential_b64 = result
            .credential
            .expect("Credential should be present on 200 OK");

        assert!(
            !credential_b64.is_empty(),
            "Encoded credential string should not be empty"
        );
    }

    #[test]
    fn userauth_submit_works_with_mock_measurement() {
        let public_params = "AYAWz7F8oKPtK+mHf/RJw2kBcQ+r5gT81HHsiM+3ZEJQAUyrROFBhwftdH6IJV69nYHy3bRuHmc27BmsJx966p80AwAAAAAAAABc8OIsTyiGCSjp3xT0rvevfKX6Qv2rg//nn9RcjBsPKzoQNZdMymjjkiOAYUxg9WgfCxN/lJvn6hcLt4a+MrJXcgXPtzlDa8cvtauhi6Um4THT+h4L/0zW3AxfmZTVw1Q=".to_string();
        let manifest_version = "DJF88g0blInW8uw4zodNNZkdOd3UcXAx".to_string();

        let open_url = format!("{BASE_URL}/report");
        let report_payload = serde_json::json!({
            "data_format_version": "0.2.0",
            "format": "json",
            "probe_asn": "AS117",
            "probe_cc": "IT",
            "software_name": "ooniprobe-engine",
            "software_version": "0.1.0",
            "test_name": "dummy",
            "test_start_time": "2019-10-28 12:51:06",
            "test_version": "0.1.0"
        })
        .to_string();

        let open_resp = client_post(
            open_url,
            vec![KeyValue {
                key: "Content-Type".into(),
                value: "application/json".into(),
            }],
            report_payload,
        )
        .expect("Failed to open report");

        let open_body: serde_json::Value =
            serde_json::from_str(open_resp.body_text.as_ref().unwrap()).unwrap();

        let report_id = open_body["report_id"].as_str().unwrap().to_string();
        println!("Opened Report ID: {}", report_id);

        let reg_result = userauth_register(
            format!("{BASE_URL}/api/v1/sign_credential"),
            public_params.clone(),
            manifest_version.clone(),
        )
        .expect("Registration failed");

        let credential = reg_result.credential.expect("No credential returned");
        println!("Registered probe with credential: {}", credential);

        let probe_cc = "IT".to_string();
        let probe_asn = "AS117".to_string();
        let probe_id = get_probe_id(credential.clone(), probe_cc.clone(), probe_asn.clone())
            .expect("No probe id returned");
        println!("Probe ID for measurement: {}", probe_id.probe_id);

        let measurement_content = serde_json::json!({
            "id": "bdd20d7a-bba5-40dd-a111-9863d7908572",
            "probe_id": probe_id.probe_id,
            "probe_asn": probe_asn,
            "probe_cc": probe_cc,
            "software_name": "ooniprobe-engine",
            "software_version": "0.1.0",
            "test_name": "dummy",
            "test_start_time": "2019-10-28 12:51:06",
            "test_version": "0.1.0",
            "test_keys": {"failure": null}
        })
        .to_string();

        let submit_url = format!("{BASE_URL}/api/v1/submit_measurement/{}", report_id);
        let submit_result = userauth_submit(
            submit_url,
            credential,
            public_params,
            measurement_content,
            probe_cc.clone(),
            probe_asn.clone(),
            manifest_version,
            2461098,
        )
        .expect("Submission call failed");

        assert_eq!(
            submit_result.response.status_code, 200,
            "Submission rejected by collector"
        );
        assert!(
            submit_result.credential.is_some(),
            "Should have received an updated credential"
        );
    }
}
