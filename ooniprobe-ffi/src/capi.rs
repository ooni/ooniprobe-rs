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
    proxy: *const c_char,
) -> ClientResponse {
    let (Some(url), Some(public_params), Some(manifest_version)) = (
        c_string_to_owned(url),
        c_string_to_owned(public_params),
        c_string_to_owned(manifest_version),
    ) else {
        return ClientResponse::err("null or invalid input".to_string());
    };

    let proxy = c_string_to_owned(proxy);

    match userauth_register_impl(url, public_params, manifest_version, proxy) {
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
    proxy: *const c_char,
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

    let proxy = c_string_to_owned(proxy);

    let credential_config = match c_string_to_owned(credential_config_json) {
        Some(raw) => match serde_json::from_str::<CredentialConfig>(&raw) {
            Ok(config) => Some(config),
            Err(e) => {
                return ClientResponse::err(format!("invalid credential config: {e}"));
            }
        },
        None => None,
    };

    match userauth_submit_impl(url, content, probe_cc, probe_asn, proxy, credential_config) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    unsafe fn read_field(ptr: *const c_char) -> Option<String> {
        if ptr.is_null() {
            None
        } else {
            Some(CStr::from_ptr(ptr).to_str().unwrap().to_owned())
        }
    }

    // Known-good public params / manifest (same fixtures the userauth tests use),
    // so `userauth_register` gets past its local crypto and issues the HTTP POST.
    const PUBLIC_PARAMS: &str = "AdqzxWc0xFMFlXygX+KfKxRGy6EEOgukeGokXmfsBA0QAUiqSrbV636keUJkvV8SfGpuD3P1sqor6w6jlTZxUIN6AwAAAAAAAADK2ygnqfhicm2pXO8Tu73Pu4AhHrJExfG1rW8uLk1UfQzxKzdpwnhmUx7qsdD9yXoy3J1B4Bh4OXMan2VfTPJVvs7JmVFr3V6iSqgoV1+RJfgQZXq5WB9439tng+4bUWs=";
    const MANIFEST_VERSION: &str = "TjxIhQyJHRZsqmidU_coSEl2dZUiBGvL";

    fn start_mock_proxy() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock proxy");
        let addr = listener.local_addr().expect("local addr");
        let hits = Arc::new(AtomicUsize::new(0));

        let hits_thread = Arc::clone(&hits);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 4096];
                let mut data = Vec::new();
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            data.extend_from_slice(&buf[..n]);
                            if data.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                hits_thread.fetch_add(1, Ordering::SeqCst);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                );
                let _ = stream.flush();
            }
        });

        (format!("http://{addr}"), hits)
    }

    /// The `proxy` parameter must thread through the C ABI: pointing it at a local
    /// mock proxy causes the outbound request to land on that proxy.
    #[test]
    fn userauth_register_routes_through_proxy() {
        let (proxy_url, hits) = start_mock_proxy();

        let url = CString::new("http://example.invalid/api/v1/sign_credential").unwrap();
        let public_params = CString::new(PUBLIC_PARAMS).unwrap();
        let manifest_version = CString::new(MANIFEST_VERSION).unwrap();
        let proxy = CString::new(proxy_url).unwrap();

        let response = unsafe {
            userauth_register(
                url.as_ptr(),
                public_params.as_ptr(),
                manifest_version.as_ptr(),
                proxy.as_ptr(),
            )
        };
        // We only assert routing; the mock's canned body isn't a valid registration
        // reply, so `response` may carry an error — either way it must be freed.
        unsafe { client_response_free(response) };

        assert_eq!(hits.load(Ordering::SeqCst), 1, "request should route through the proxy");
    }

    /// A null `proxy` pointer is valid and maps to `None` (no proxy).
    #[test]
    fn userauth_register_accepts_null_proxy() {
        let (_proxy_url, hits) = start_mock_proxy();

        let url = CString::new("http://example.invalid/api/v1/sign_credential").unwrap();
        let public_params = CString::new(PUBLIC_PARAMS).unwrap();
        let manifest_version = CString::new(MANIFEST_VERSION).unwrap();

        let response = unsafe {
            userauth_register(
                url.as_ptr(),
                public_params.as_ptr(),
                manifest_version.as_ptr(),
                ptr::null(),
            )
        };
        unsafe { client_response_free(response) };

        // With no proxy configured, the mock proxy must not be contacted.
        assert_eq!(hits.load(Ordering::SeqCst), 0, "null proxy must not route through the mock");
    }

    #[test]
    fn register_with_null_required_input_returns_error() {
        let public_params = CString::new(PUBLIC_PARAMS).unwrap();
        let manifest_version = CString::new(MANIFEST_VERSION).unwrap();

        // A null `url` (required) must yield an error response, never a panic.
        let response = unsafe {
            userauth_register(
                ptr::null(),
                public_params.as_ptr(),
                manifest_version.as_ptr(),
                ptr::null(),
            )
        };
        let error = unsafe { read_field(response.error) };
        let json = unsafe { read_field(response.json) };
        unsafe { client_response_free(response) };

        assert!(json.is_none(), "json should be null on error");
        assert_eq!(error.as_deref(), Some("null or invalid input"));
    }

    #[test]
    fn register_with_invalid_public_params_returns_error() {
        // Invalid base64 public params fails during local decoding, before any
        // network call — so this is fully offline and deterministic.
        let url = CString::new("http://example.invalid/api/v1/sign_credential").unwrap();
        let public_params = CString::new("not-valid-base64-!!!").unwrap();
        let manifest_version = CString::new(MANIFEST_VERSION).unwrap();

        let response = unsafe {
            userauth_register(
                url.as_ptr(),
                public_params.as_ptr(),
                manifest_version.as_ptr(),
                ptr::null(),
            )
        };
        let error = unsafe { read_field(response.error) };
        let json = unsafe { read_field(response.json) };
        unsafe { client_response_free(response) };

        assert!(json.is_none(), "json should be null on error");
        assert!(
            error.is_some_and(|e| e.contains("decode")),
            "expected a decode error"
        );
    }

    #[test]
    fn submit_without_credential_routes_through_proxy() {
        // With a null credential config, `userauth_submit` just posts the content.
        // Against the mock proxy's `200 {}` reply it returns a success response
        // (no updated credential), which proves the proxy path end-to-end.
        let (proxy_url, hits) = start_mock_proxy();

        let url = CString::new("http://example.invalid/api/v1/submit_measurement").unwrap();
        let content = CString::new("{}").unwrap();
        let probe_cc = CString::new("IT").unwrap();
        let probe_asn = CString::new("AS117").unwrap();
        let proxy = CString::new(proxy_url).unwrap();

        let response = unsafe {
            userauth_submit(
                url.as_ptr(),
                content.as_ptr(),
                probe_cc.as_ptr(),
                probe_asn.as_ptr(),
                proxy.as_ptr(),
                ptr::null(),
            )
        };
        let error = unsafe { read_field(response.error) };
        let json = unsafe { read_field(response.json) };
        unsafe { client_response_free(response) };

        assert_eq!(hits.load(Ordering::SeqCst), 1, "request should route through the proxy");
        assert!(error.is_none(), "unexpected error: {error:?}");
        assert!(json.is_some(), "expected a JSON payload");
    }

    #[test]
    fn submit_with_invalid_credential_config_returns_error() {
        // Malformed credential-config JSON is rejected before any network call.
        let url = CString::new("http://example.invalid/api/v1/submit_measurement").unwrap();
        let content = CString::new("{}").unwrap();
        let probe_cc = CString::new("IT").unwrap();
        let probe_asn = CString::new("AS117").unwrap();
        let bad_config = CString::new("{ not valid json").unwrap();

        let response = unsafe {
            userauth_submit(
                url.as_ptr(),
                content.as_ptr(),
                probe_cc.as_ptr(),
                probe_asn.as_ptr(),
                ptr::null(),
                bad_config.as_ptr(),
            )
        };
        let error = unsafe { read_field(response.error) };
        let json = unsafe { read_field(response.json) };
        unsafe { client_response_free(response) };

        assert!(json.is_none(), "json should be null on error");
        assert!(
            error.is_some_and(|e| e.contains("invalid credential config")),
            "expected an invalid-credential-config error"
        );
    }

    #[test]
    fn get_probe_id_with_null_input_returns_error() {
        let probe_asn = CString::new("AS117").unwrap();
        let probe_cc = CString::new("IT").unwrap();

        let response =
            unsafe { get_probe_id(ptr::null(), probe_asn.as_ptr(), probe_cc.as_ptr()) };
        let error = unsafe { read_field(response.error) };
        let json = unsafe { read_field(response.json) };
        unsafe { client_response_free(response) };

        assert!(json.is_none(), "json should be null on error");
        assert_eq!(error.as_deref(), Some("null or invalid input"));
    }

    #[test]
    fn get_probe_id_with_invalid_credential_returns_error() {
        let credential = CString::new("not-valid-base64-!!!").unwrap();
        let probe_asn = CString::new("AS117").unwrap();
        let probe_cc = CString::new("IT").unwrap();

        let response = unsafe {
            get_probe_id(credential.as_ptr(), probe_asn.as_ptr(), probe_cc.as_ptr())
        };
        let error = unsafe { read_field(response.error) };
        let json = unsafe { read_field(response.json) };
        unsafe { client_response_free(response) };

        assert!(json.is_none(), "json should be null on error");
        assert!(error.is_some(), "expected an error for an invalid credential");
    }

    #[test]
    fn client_response_free_handles_null_fields() {
        // Freeing an all-null response must be a safe no-op.
        let response = ClientResponse {
            json: ptr::null_mut(),
            error: ptr::null_mut(),
        };
        unsafe { client_response_free(response) };
    }
}
