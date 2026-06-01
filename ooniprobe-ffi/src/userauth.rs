use std::ops::Range;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bincode;
use cmz::cmz_group_init;
use curve25519_dalek::{ristretto::RistrettoPoint as G};
use ooniauth_core::{
    registration::UserAuthCredential,
    submit::{digest_point, submit_measurement_hash}
};
use serde::{Deserialize, Serialize};
use sha2::{Sha512};

use crate::client::build_client;
use crate::errors::OoniError;
use crate::HttpResponse;
use ooniauth_core::{PublicParameters, VERSION};

#[derive(Clone, Debug)]
pub struct ParamRange {
    pub min: u32,
    pub max: u32,
}

#[derive(Clone, Debug)]
pub struct CredentialConfig {
    pub credential: String,
    pub public_params: String,
    pub manifest_version: String,
    pub age_range: ParamRange,
    pub measurement_count_range: ParamRange,
}

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
    emission_day: u32,
}

#[derive(Serialize, Deserialize)]
struct SubmitMeasurementPayload {
    format: String,
    content: String,
    nym: Option<String>,
    zkp_request: Option<String>,
    manifest_version: Option<String>,
    protocol_version: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SubmitMeasurementResponse {
    measurement_uid: Option<String>,
    verification_status: String,
    submit_response: Option<String>,
    protocol_version: String,
    error: Option<String>
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
    let reply = bincode::deserialize::<ooniauth_core::registration::open_registration::Reply>(
        &reply_bytes,
    )?;

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
    content: String,
    probe_cc: String,
    probe_asn: String,
    credential_config: Option<CredentialConfig>,
) -> Result<CredentialResult, OoniError> {
    let (submit_payload, auth_state) = match credential_config {
        Some(config) => {
            // Initialize user state
            let pp = decode_public_params(&config.public_params)?;
            let mut user_state = ooniauth_core::UserState::new(pp);

            let credential = decode_credential(&config.credential)?;
            user_state.set_credential(credential);

            let measurement_hash = submit_measurement_hash(content.as_bytes()); 

            // Create submit request
            let mut rng = rand::thread_rng();
            let ((submit_request, submit_state), probe_id) = user_state.submit_request(
                &mut rng,
                probe_cc.clone(),
                probe_asn.clone(),
                &measurement_hash,
                Range {
                    start: config.age_range.min,
                    end: config.age_range.max,
                },
                Range {
                    start: config.measurement_count_range.min,
                    end: u32::MAX
                },
            )?;

            let request_bytes = submit_request.as_bytes();

            let payload = SubmitMeasurementPayload {
                format: "json".to_string(),
                content: content,
                nym: Some(b64_encode(&probe_id)),
                zkp_request: Some(b64_encode(&request_bytes)),
                manifest_version: Some(config.manifest_version),
                protocol_version: Some(VERSION.to_string()),
            };

            (payload, Some((user_state, submit_state)))
        }
        None => {
            let payload = SubmitMeasurementPayload {
                format: "json".to_string(),
                content: content,
                nym: None,
                zkp_request: None,
                manifest_version: None,
                protocol_version: None,
            };

            (payload, None)
        }
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

    // return early if the submission path was without credentials
    let Some((mut user_state, submit_state)) = auth_state else {
        return Ok(CredentialResult {
            credential: None,
            response,
        });
    };

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
    use crate::get_probe_id;
    use crate::userauth::{userauth_register, userauth_submit, CredentialConfig, ParamRange};

    const BASE_URL: &str = "https://api.dev.ooni.io";

    #[test]
    fn userauth_register_works_with_public_params() {
        let url = format!("{BASE_URL}/api/v1/sign_credential");

        let public_params = "AdqzxWc0xFMFlXygX+KfKxRGy6EEOgukeGokXmfsBA0QAUiqSrbV636keUJkvV8SfGpuD3P1sqor6w6jlTZxUIN6AwAAAAAAAADK2ygnqfhicm2pXO8Tu73Pu4AhHrJExfG1rW8uLk1UfQzxKzdpwnhmUx7qsdD9yXoy3J1B4Bh4OXMan2VfTPJVvs7JmVFr3V6iSqgoV1+RJfgQZXq5WB9439tng+4bUWs=";
        let manifest_version = "TjxIhQyJHRZsqmidU_coSEl2dZUiBGvL";

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
        let public_params = "AdqzxWc0xFMFlXygX+KfKxRGy6EEOgukeGokXmfsBA0QAUiqSrbV636keUJkvV8SfGpuD3P1sqor6w6jlTZxUIN6AwAAAAAAAADK2ygnqfhicm2pXO8Tu73Pu4AhHrJExfG1rW8uLk1UfQzxKzdpwnhmUx7qsdD9yXoy3J1B4Bh4OXMan2VfTPJVvs7JmVFr3V6iSqgoV1+RJfgQZXq5WB9439tng+4bUWs=".to_string();
        let manifest_version = "TjxIhQyJHRZsqmidU_coSEl2dZUiBGvL".to_string();

        let reg_result = userauth_register(
            format!("{BASE_URL}/api/v1/sign_credential"),
            public_params.clone(),
            manifest_version.clone(),
        )
        .expect("Registration failed");

        let credential = reg_result.credential.expect("No credential returned");

        let probe_cc = "IT".to_string();
        let probe_asn = "AS117".to_string();
        let probe_id = get_probe_id(credential.clone(), probe_cc.clone(), probe_asn.clone())
            .expect("No probe id returned");

        let measurement_content = serde_json::json!({
            "id": "bdd20d7a-bba5-40dd-a111-9863d7908572",
            "measurement_start_time": "2026-03-31 23:59:58",
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

        let submit_url = format!("{BASE_URL}/api/v1/submit_measurement");
        let credential_config = Some(CredentialConfig {
            credential: credential,
            public_params: public_params,
            manifest_version: manifest_version,
            age_range: ParamRange { min: 2461110, max: 2826140 },
            measurement_count_range: ParamRange { min: 0, max: 10000000 },
        });
        let submit_result = userauth_submit(
            submit_url,
            measurement_content,
            probe_cc.clone(),
            probe_asn.clone(),
            credential_config,
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
