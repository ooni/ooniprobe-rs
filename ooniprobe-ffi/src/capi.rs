use std::ffi::{c_char, CStr, CString};
use std::ptr;

use serde_json::json;

use crate::userauth::{
    get_probe_id as get_probe_id_impl, userauth_register as userauth_register_impl,
    userauth_submit as userauth_submit_impl, CredentialConfig,
};

/// Flat C-ABI result carrying either a JSON payload or an error string.
///
/// Exactly one of `json` / `error` is non-null on return. Both pointers are
/// owned by the callee and must be freed via [`client_response_free`].
#[repr(C)]
pub struct ClientResponse {
    pub json: *mut c_char,
    pub error: *mut c_char,
}

impl ClientResponse {
    fn ok(payload: String) -> Self {
        ClientResponse {
            json: into_c_string(payload),
            error: ptr::null_mut(),
        }
    }

    fn err(message: String) -> Self {
        ClientResponse {
            json: ptr::null_mut(),
            error: into_c_string(message),
        }
    }
}

fn into_c_string(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(value) => value.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

unsafe fn c_string_to_owned(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_owned())
}

/// Register and obtain an initial credential.
#[no_mangle]
pub unsafe extern "C" fn userauth_register(
    url: *const c_char,
    public_params: *const c_char,
    manifest_version: *const c_char,
) -> ClientResponse {
    let (Some(url), Some(public_params), Some(manifest_version)) = (
        c_string_to_owned(url),
        c_string_to_owned(public_params),
        c_string_to_owned(manifest_version),
    ) else {
        return ClientResponse::err("null or invalid input".to_string());
    };

    match userauth_register_impl(url, public_params, manifest_version) {
        Ok(result) => {
            let payload = json!({
                "credential": result.credential,
                "status_code": result.response.status_code,
                "body": result.response.body_text,
            });
            ClientResponse::ok(payload.to_string())
        }
        Err(e) => ClientResponse::err(e.to_string()),
    }
}

/// Submit a measurement, optionally authenticated with a credential.
#[no_mangle]
pub unsafe extern "C" fn userauth_submit(
    url: *const c_char,
    content: *const c_char,
    probe_cc: *const c_char,
    probe_asn: *const c_char,
    credential_config_json: *const c_char,
) -> ClientResponse {
    let (Some(url), Some(content), Some(probe_cc), Some(probe_asn)) = (
        c_string_to_owned(url),
        c_string_to_owned(content),
        c_string_to_owned(probe_cc),
        c_string_to_owned(probe_asn),
    ) else {
        return ClientResponse::err("null or invalid input".to_string());
    };

    let credential_config = match c_string_to_owned(credential_config_json) {
        Some(raw) => match serde_json::from_str::<CredentialConfig>(&raw) {
            Ok(config) => Some(config),
            Err(e) => {
                return ClientResponse::err(format!("invalid credential config: {e}"));
            }
        },
        None => None,
    };

    match userauth_submit_impl(url, content, probe_cc, probe_asn, credential_config) {
        Ok(result) => {
            let payload = json!({
                "credential": result.credential,
                "status_code": result.response.status_code,
                "body": result.response.body_text,
            });
            ClientResponse::ok(payload.to_string())
        }
        Err(e) => ClientResponse::err(e.to_string()),
    }
}

/// Derive the hex-encoded probe id from a credential.
#[no_mangle]
pub unsafe extern "C" fn get_probe_id(
    credential_b64: *const c_char,
    probe_asn: *const c_char,
    probe_cc: *const c_char,
) -> ClientResponse {
    let (Some(credential_b64), Some(probe_asn), Some(probe_cc)) = (
        c_string_to_owned(credential_b64),
        c_string_to_owned(probe_asn),
        c_string_to_owned(probe_cc),
    ) else {
        return ClientResponse::err("null or invalid input".to_string());
    };

    match get_probe_id_impl(credential_b64, probe_asn, probe_cc) {
        Ok(result) => {
            let payload = json!({ "probe_id": result.probe_id });
            ClientResponse::ok(payload.to_string())
        }
        Err(e) => ClientResponse::err(e.to_string()),
    }
}

/// Free the memory owned by a [`ClientResponse`].
#[no_mangle]
pub unsafe extern "C" fn client_response_free(response: ClientResponse) {
    if !response.json.is_null() {
        drop(CString::from_raw(response.json));
    }
    if !response.error.is_null() {
        drop(CString::from_raw(response.error));
    }
}
