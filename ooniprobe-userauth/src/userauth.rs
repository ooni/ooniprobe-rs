use curve25519_dalek::Scalar;
use base64::prelude::BASE64_STANDARD;
use bincode;
use rand;
use serde::{Serialize, Deserialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use base64::Engine;

use ooniauth_core::PublicParameters;
use ooniprobe_services::client::Client;

#[repr(C)]
pub struct ClientResponse {
    pub json: *mut c_char,
    pub error: *mut c_char,
}

impl ClientResponse {
    fn success(json: String) -> Self {
        match CString::new(json) {
            Ok(cstr) => Self {
                json: cstr.into_raw(),
                error: ptr::null_mut(),
            },
            Err(_) => Self::error("response contains interior null byte"),
        }
    }

    fn error<E: std::fmt::Debug>(e: E) -> Self {
        let msg = format!("{:?}", e);
        let cstr = CString::new(msg)
            .unwrap_or_else(|_| CString::new("invalid error").unwrap());

        Self {
            json: ptr::null_mut(),
            error: cstr.into_raw(),
        }
    }
}

/// Base64 encode bytes to string
fn b64_encode(b: &[u8]) -> String {
    BASE64_STANDARD.encode(b)
}

/// Base64 decode string to bytes
fn b64_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    BASE64_STANDARD.decode(s)
}

/// Convert a raw C string pointer into a Rust `String`.
/// Returns `None` if the pointer is null or invalid UTF-8.
fn c_char_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string()) }
}

/// Get today's date as Julian day number (u32)
fn today() -> u32 {
    time::OffsetDateTime::now_utc()
        .date()
        .to_julian_day()
        .try_into()
        .expect("Julian day should fit in u32")
}

/// Decode base64-encoded public parameters
fn decode_public_params(
    public_params: *const c_char,
) -> Result<PublicParameters, String> {
    let s = c_char_to_string(public_params)
        .ok_or("public_params is null or invalid UTF-8")?;

    let bytes = b64_decode(&s).map_err(|e| format!("base64 decode failed: {:?}", e))?;

    bincode::deserialize(&bytes).map_err(|e| format!("bincode deserialize failed: {:?}", e))
}

/// Build HTTP client with error handling
fn build_client() -> Result<Client, String> {
    Client::builder()
        .build()
        .map_err(|e| format!("failed to build client: {:?}", e))
}


/// Free memory allocated by ClientResponse
/// 
/// # Safety
/// This function must be called exactly once for each ClientResponse
/// returned by other FFI functions to avoid memory leaks.
#[no_mangle]
pub extern "C" fn client_response_free(response: ClientResponse) {
    if !response.json.is_null() {
        unsafe { drop(CString::from_raw(response.json)) };
    }
    if !response.error.is_null() {
        unsafe { drop(CString::from_raw(response.error)) };
    }
}


/// Perform HTTP GET request
/// 
/// # Safety
/// - `url` must be a valid null-terminated C string
/// - Caller must call `client_response_free` on the returned value
#[no_mangle]
pub extern "C" fn client_get(url: *const c_char) -> ClientResponse {
    match client_get_impl(url) {
        Ok(json) => ClientResponse::success(json),
        Err(e) => ClientResponse::error(e),
    }
}

fn client_get_impl(url: *const c_char) -> Result<String, String> {
    let url = c_char_to_string(url).ok_or("url is null or invalid UTF-8")?;
    let client = build_client()?;

    let request = client
        .request("GET", &url)
        .and_then(|b| b.build().map_err(Into::into))
        .map_err(|e| format!("failed to build request: {:?}", e))?;

    let response = client
        .execute(request)
        .map_err(|e| format!("failed to execute request: {:?}", e))?;

    response
        .to_json_str()
        .map_err(|e| format!("failed to read response body: {:?}", e))
}


/// Perform HTTP POST request
/// 
/// # Safety
/// - `url` and `payload` must be valid null-terminated C strings
/// - Caller must call `client_response_free` on the returned value
#[no_mangle]
pub extern "C" fn client_post(url: *const c_char, payload: *const c_char) -> ClientResponse {
    match client_post_impl(url, payload) {
        Ok(json) => ClientResponse::success(json),
        Err(e) => ClientResponse::error(e),
    }
}

fn client_post_impl(url: *const c_char, payload: *const c_char) -> Result<String, String> {
    let url = c_char_to_string(url).ok_or("url is null or invalid UTF-8")?;
    let payload = c_char_to_string(payload).ok_or("payload is null or invalid UTF-8")?;
    let client = build_client()?;

    let request = client
        .request("POST", &url)
        .map(|b| b.header("Content-Type", "application/json").body(payload))
        .and_then(|b| b.build().map_err(Into::into))
        .map_err(|e| format!("failed to build request: {:?}", e))?;

    let response = client
        .execute(request)
        .map_err(|e| format!("failed to execute request: {:?}", e))?;

    response
        .to_json_str()
        .map_err(|e| format!("failed to read response body: {:?}", e))
}

#[derive(Serialize, Deserialize)]
struct RegistrationPayload {
    manifest_version: String,
    credential_sign_request: String,
}

#[derive(Serialize, Deserialize)]
struct RegistrationResponse {
    credential_sign_response: String,
    emission_day: i16,
}


/// Register a user and obtain a credential
/// 
/// # Safety
/// - All parameters must be valid null-terminated C strings
/// - Caller must call `client_response_free` on the returned value
#[no_mangle]
pub extern "C" fn userauth_register(
    url: *const c_char,
    public_params: *const c_char,
    manifest_version: *const c_char,
) -> ClientResponse {
    match userauth_register_impl(url, public_params, manifest_version) {
        Ok(json) => ClientResponse::success(json),
        Err(e) => ClientResponse::error(e),
    }
}

fn userauth_register_impl(
    url: *const c_char,
    public_params: *const c_char,
    manifest_version: *const c_char,
) -> Result<String, String> {
    // Parse and validate inputs
    let url = c_char_to_string(url).ok_or("url is null or invalid UTF-8")?;
    let manifest_version = c_char_to_string(manifest_version)
        .ok_or("manifest_version is null or invalid UTF-8")?;
    let pp = decode_public_params(public_params)?;

    // Create registration request
    let mut rng = rand::thread_rng();
    let (reg_request, reg_state) = ooniauth_core::user_registration::request(&pp, &mut rng)
        .map_err(|e| format!("failed to create registration request: {:?}", e))?;

    let request_bytes = reg_request.as_bytes();
    let request_payload = b64_encode(&request_bytes);

    // Build payload
    let payload = RegistrationPayload {
        manifest_version,
        credential_sign_request: request_payload,
    };

    let json_payload = serde_json::to_string(&payload)
        .map_err(|e| format!("failed to serialize payload: {:?}", e))?;

    // Execute HTTP request
    let client = build_client()?;
    let request = client
        .request("POST", &url)
        .map(|b| b.body(json_payload))
        .and_then(|b| b.build().map_err(Into::into))
        .map_err(|e| format!("failed to build request: {:?}", e))?;

    let response = client
        .execute(request)
        .map_err(|e| format!("failed to execute request: {:?}", e))?;

    let body = response
        .to_json_str()
        .map_err(|e| format!("failed to read response body: {:?}", e))?;

    // Parse response
    let resp: RegistrationResponse = serde_json::from_str(&body)
        .map_err(|e| format!("invalid JSON response: {:?}", e))?;

    let reply_bincode_bytes = b64_decode(&resp.credential_sign_response)
        .map_err(|e| format!("invalid base64 in response: {:?}", e))?;

    let reply: ooniauth_core::registration::open_registration::Reply =
        bincode::deserialize(&reply_bincode_bytes).map_err(|e| {
            format!("failed to deserialize registration reply (bincode): {:?}", e)
        })?;

    // Handle registration response
    let credential = ooniauth_core::user_registration::handle_request_response(reg_state, reply)
        .map_err(|e| format!("credential verification failed: {:?}", e))?;

    // Validate credential
    if credential.nym_id == Some(Scalar::ZERO) {
        return Err("invalid credential: nym_id is zero".to_string());
    }

    // Build output JSON
    let out_json = serde_json::to_string(&serde_json::json!({
        "credential": resp.credential_sign_response,
        "emission_day": resp.emission_day,
    }))
    .map_err(|e| format!("failed to serialize output: {:?}", e))?;

    Ok(out_json)
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


/// Submit user credentials with measurement data
/// 
/// # Safety
/// - All parameters must be valid null-terminated C strings
/// - `credential_b64` must be a valid base64-encoded credential
/// - `public_params` must be valid base64 public parameters
/// - Caller must call `client_response_free` on the returned value
#[no_mangle]
pub extern "C" fn userauth_submit(
    url: *const c_char,
    credential_b64: *const c_char,
    public_params: *const c_char,
    probe_cc: *const c_char,
    probe_asn: *const c_char,
    manifest_version: *const c_char,
) -> ClientResponse {
    match userauth_submit_impl(url, credential_b64, public_params, probe_cc, probe_asn, manifest_version) {
        Ok(json) => ClientResponse::success(json),
        Err(e) => ClientResponse::error(e),
    }
}

fn userauth_submit_impl(
    url: *const c_char,
    credential_b64: *const c_char,
    public_params: *const c_char,
    probe_cc: *const c_char,
    probe_asn: *const c_char,
    manifest_version: *const c_char,
) -> Result<String, String> {
    // Parse inputs
    let url_str = c_char_to_string(url).ok_or("url is null or invalid UTF-8")?;
    let probe_cc_str = c_char_to_string(probe_cc).ok_or("probe_cc is null or invalid UTF-8")?;
    let probe_asn_str = c_char_to_string(probe_asn).ok_or("probe_asn is null or invalid UTF-8")?;
    let credential_b64_str = c_char_to_string(credential_b64).ok_or("credential is null or invalid UTF-8")?;
    let manifest_version_str = c_char_to_string(manifest_version).ok_or("manifest_version is null or invalid UTF-8")?;

    let pp = decode_public_params(public_params)
        .map_err(|e| format!("failed to decode public params: {:?}", e))?;

    let cred_bytes = b64_decode(&credential_b64_str)
        .map_err(|e| format!("invalid credential base64: {:?}", e))?;
    let credential: ooniauth_core::registration::UserAuthCredential =
        bincode::deserialize(&cred_bytes)
            .map_err(|e| format!("invalid credential encoding: {:?}", e))?;

    // Setup ranges
    // Note: Today is generated client-side but may be overwritten by server
    let today = today();
    let age_range = (today - 30)..(today + 1);
    let measurement_count_range = 0..100;

    // Create submit request
    let mut rng = rand::thread_rng();
    let ((submit_request, submit_state), nym) = ooniauth_core::user_submit::submit_request(
            &credential,
            &pp,
            &mut rng,
            probe_cc_str.clone(),
            probe_asn_str.clone(),
            age_range.clone(),
            measurement_count_range.clone(),
        )
        .map_err(|e| format!("failed to create submit request: {:?}", e))?;

    let submit_payload = SubmitMeasurementPayload {
        format: "json".to_string(),
        content: SubmitContent {
            probe_cc: probe_cc_str.clone(),
            probe_asn: probe_asn_str.clone(),
        },
        nym: BASE64_STANDARD.encode(nym),
        zkp_request: BASE64_STANDARD.encode(submit_request.as_bytes()),
        probe_age_range: (age_range.start, age_range.end),
        probe_msm_range: (measurement_count_range.start, measurement_count_range.end),
        manifest_version: manifest_version_str,
    };

    let json_payload = serde_json::to_string(&submit_payload)
        .map_err(|e| format!("failed to serialize submit payload: {:?}", e))?;

    // Build and execute HTTP request
    let client = build_client()?;
    let request = client
        .request("POST", &url_str)
        .map(|b| b.body(json_payload))
        .and_then(|b| b.build().map_err(Into::into))
        .map_err(|e| format!("failed to build request: {:?}", e))?;

    let response = client
        .execute(request)
        .map_err(|e| format!("failed to execute request: {:?}", e))?;

    let body = response
        .to_json_str()
        .map_err(|e| format!("failed to read response body: {:?}", e))?;
    
    // Parse response
    let resp: SubmitMeasurementResponse = serde_json::from_str(&body)
        .map_err(|e| format!("invalid JSON response: {:?}", e))?;

    let reply_bincode_bytes = b64_decode(&resp.submit_response)
        .map_err(|e| format!("invalid base64 in response: {:?}", e))?;

    let reply: ooniauth_core::submit::submit::Reply =
        bincode::deserialize(&reply_bincode_bytes).map_err(|e| {
            format!("failed to deserialize submit reply (bincode): {:?}", e)
        })?;

    // Handle submit response
    let updated_credential = ooniauth_core::user_submit::handle_submit_response(submit_state, reply)
        .map_err(|e| format!("credential verification failed: {:?}", e))?;

    // Serialize updated credential for return
    let updated_cred_bytes = bincode::serialize(&updated_credential)
        .map_err(|e| format!("failed to serialize updated credential: {:?}", e))?;

    let updated_cred_b64 = b64_encode(&updated_cred_bytes);

    let out = SubmitMeasurementResponse {
        measurement_uid: resp.measurement_uid, // passthrough from server
        is_verified: resp.is_verified,
        submit_response: updated_cred_b64,
    };

    let out_json = serde_json::to_string(&out)
        .map_err(|e| format!("failed to serialize output: {:?}", e))?;

    Ok(out_json)
}
