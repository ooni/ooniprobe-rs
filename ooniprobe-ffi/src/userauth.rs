use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bincode;
use cmz::*;
use curve25519_dalek::ristretto::RistrettoPoint as G;
use curve25519_dalek::Scalar;
use rand;
use serde::{Deserialize, Serialize};
use sha2::Sha512;

use ooniauth_core::PublicParameters;
use crate::errors::OoniError;
use crate::client::build_client;

#[derive(Debug)]
pub struct RegistrationResult {
    pub credential: String,
    pub emission_day: i32,
}

#[derive(Debug)]
pub struct SubmitResult {
    pub measurement_uid: Option<String>,
    pub is_verified: bool,
    pub updated_credential: String,
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
struct SubmitContent {
    probe_cc: String,
    probe_asn: String,
}

#[derive(Serialize, Deserialize)]
struct SubmitMeasurementPayload {
    format: String,
    content: SubmitContent,
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
    submit_response: String,
}

fn b64_encode(b: &[u8]) -> String {
    BASE64_STANDARD.encode(b)
}

fn b64_decode(s: &str) -> Result<Vec<u8>, OoniError> {
    BASE64_STANDARD
        .decode(s)
        .map_err(|e| OoniError::Base64DecodeError(format!("{:?}", e)))
}

fn today() -> u32 {
    time::OffsetDateTime::now_utc()
        .date()
        .to_julian_day()
        .try_into()
        .expect("Julian day should fit in u32")
}

fn decode_public_params(public_params: &str) -> Result<PublicParameters, OoniError> {
    cmz_group_init(G::hash_from_bytes::<Sha512>(b"CMZ Generator A"));
    let raw = b64_decode(public_params)?;
    let pubkey: CMZPubkey<G> = bincode::deserialize(&raw)
        .map_err(|e| OoniError::BincodeDecodeError(format!("{:?}", e)))?;

    Ok(pubkey)
}

pub fn userauth_register(
    url: String,
    public_params: String,
    manifest_version: String,
) -> Result<RegistrationResult, OoniError> {
    let pp = decode_public_params(&public_params)?;

    let mut rng = rand::thread_rng();
    let (reg_request, reg_state) = ooniauth_core::user_registration::request(&pp, &mut rng)
        .map_err(|e| OoniError::CryptoError(format!("{:?}", e)))?;

    let raw_bytes = reg_request.as_bytes();
    let request_payload = b64_encode(&raw_bytes);

    let payload = RegistrationPayload {
        manifest_version,
        credential_sign_request: request_payload,
    };
    let json_payload = serde_json::to_string(&payload)
        .map_err(|e| OoniError::SerializationError(format!("{:?}", e)))?;

    let client = build_client()?;
    let request = client
        .request("POST", &url)
        .map(|b| b.body(json_payload))
        .and_then(|b| b.build().map_err(Into::into))
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    let response = client
        .execute(request)
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    let body_text = response.body_text.as_ref().expect("body_text missing");

    let resp: RegistrationResponse = serde_json::from_str(body_text)
        .map_err(|e| OoniError::SerializationError(format!("invalid JSON: {:?}", e)))?;

    let reply_bytes = b64_decode(&resp.credential_sign_response)?;
    let reply = bincode::deserialize::<ooniauth_core::registration::open_registration::Reply>(&reply_bytes)
            .map_err(|e| OoniError::BincodeDecodeError(format!("{:?}", e)))?;

    let credential = ooniauth_core::user_registration::handle_request_response(reg_state, reply)
        .map_err(|e| OoniError::CryptoError(format!("{:?}", e)))?;

    if credential.nym_id == Some(Scalar::ZERO) {
        return Err(OoniError::InvalidCredential("nym_id is zero".to_string()));
    }

    // Serialize the full credential object so the caller can persist it
    let cred_bytes = bincode::serialize(&credential)
        .map_err(|e| OoniError::SerializationError(format!("{:?}", e)))?;

    Ok(RegistrationResult {
        credential: b64_encode(&cred_bytes),
        emission_day: resp.emission_day as i32,
    })
}

pub fn userauth_submit(
    url: String,
    credential_b64: String,
    public_params: String,
    probe_cc: String,
    probe_asn: String,
    manifest_version: String,
) -> Result<SubmitResult, OoniError> {
    let pp = decode_public_params(&public_params)?;

    let cred_bytes = b64_decode(&credential_b64)?;
    let credential: ooniauth_core::registration::UserAuthCredential =
        bincode::deserialize(&cred_bytes)
            .map_err(|e| OoniError::BincodeDecodeError(format!("{:?}", e)))?;

    let today = today();
    let age_range = (today - 30)..(today + 1);
    let measurement_count_range = 0u32..100u32;

    let mut rng = rand::thread_rng();
    let ((submit_request, submit_state), nym) = ooniauth_core::user_submit::submit_request(
        &credential,
        &pp,
        &mut rng,
        probe_cc.clone(),
        probe_asn.clone(),
        age_range.clone(),
        measurement_count_range.clone(),
    )
    .map_err(|e| OoniError::CryptoError(format!("{:?}", e)))?;

    let submit_payload = SubmitMeasurementPayload {
        format: "json".to_string(),
        content: SubmitContent {
            probe_cc: probe_cc.clone(),
            probe_asn: probe_asn.clone(),
        },
        nym: b64_encode(&nym),
        zkp_request: b64_encode(&submit_request.as_bytes()),
        probe_age_range: (age_range.start, age_range.end),
        probe_msm_range: (measurement_count_range.start, measurement_count_range.end),
        manifest_version,
    };

    let json_payload = serde_json::to_string(&submit_payload)
        .map_err(|e| OoniError::SerializationError(format!("{:?}", e)))?;

    let client = build_client()?;
    let request = client
        .request("POST", &url)
        .map(|b| b.body(json_payload))
        .and_then(|b| b.build().map_err(Into::into))
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    let response = client
        .execute(request)
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    let body = response
        .to_json_str()
        .map_err(|e| OoniError::HttpClientError(format!("{:?}", e)))?;

    let resp: SubmitMeasurementResponse = serde_json::from_str(&body)
        .map_err(|e| OoniError::SerializationError(format!("invalid JSON: {:?}", e)))?;

    let reply_bytes = b64_decode(&resp.submit_response)?;
    let reply: ooniauth_core::submit::submit::Reply = bincode::deserialize(&reply_bytes)
        .map_err(|e| OoniError::BincodeDecodeError(format!("{:?}", e)))?;

    let updated_credential =
        ooniauth_core::user_submit::handle_submit_response(submit_state, reply)
            .map_err(|e| OoniError::CryptoError(format!("{:?}", e)))?;

    let updated_cred_bytes = bincode::serialize(&updated_credential)
        .map_err(|e| OoniError::SerializationError(format!("{:?}", e)))?;

    Ok(SubmitResult {
        measurement_uid: resp.measurement_uid,
        is_verified: resp.is_verified,
        updated_credential: b64_encode(&updated_cred_bytes),
    })
}

// #[cfg(test)]
// mod tests {
    // #[test]
    // fn userauth_register_works_with_manifest() {
        // const MANIFEST_URL: &str = "https://ooniprobe.dev.ooni.io/api/v1/manifest";

        // const REGISTER_URL: &str = "https://ooniprobe.dev.ooni.io/api/v1/sign_credential";

        // assert_eq!(manifest_resp.status_code, 200);

        // let body_text = manifest_resp.body_text.as_ref().expect("body_text missing");

        // let parsed: serde_json::Value =
            // serde_json::from_str(body_text).expect("response body should be valid JSON");

        // let public_params = parsed
            // .get("manifest")
            // .and_then(|m| m.get("public_parameters"))
            // .and_then(|v| v.as_str())
            // .expect("public_parameters missing")
            // .to_string();

        // let manifest_version = parsed
            // .get("meta")
            // .and_then(|m| m.get("version"))
            // .and_then(|v| v.as_str())
            // .expect("meta.version missing")
            // .to_string();

        // let result = userauth_register(REGISTER_URL.to_string(), public_params, manifest_version)
            // .expect("userauth_register should succeed");

        // assert!(
            // !result.credential.is_empty(),
            // "credential should not be empty"
        // );

        // assert!(result.emission_day > 0, "emission_day should be positive");
    // }
// }
