use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::thread::Builder;

use ooniauth_core::UserState;
use serde_json::json;

use ooniprobe_services::client::{Client, Response};
use ooniauth_core::scalar_u32;

#[repr(C)]
pub struct UserAuthResult {
    pub user: *mut UserState,
    pub error: *mut c_char
}

// Convert an error to a json error string
fn err_json<E: std::fmt::Debug>(e: E) -> *mut c_char {
    let s = json!({ "error": format!("{:?}", e) }).to_string();
    CString::new(s).unwrap().into_raw()
}

// Convert a raw C string pointer into a Rust `String`.
/// Returns `None` if the pointer is null or invalid UTF-8.
pub fn c_char_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe {
        CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
    }
}

// Get today's (real or simulated) date as u32
pub fn today() -> u32 {
    // We will not encounter negative Julian dates (~6700 years ago)
    // or ones larger than 32 bits
    (time::OffsetDateTime::now_utc().date())
        .to_julian_day()
        .try_into()
        .unwrap()
}

#[no_mangle]
pub extern "C" fn userauth_register(url: *const c_char) -> UserAuthResult {
    let url_str = c_char_to_string(url).unwrap_or_default();

    let client = match Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            let err = CString::new(format!("failed to build client: {}", e)).unwrap();
            return UserAuthResult {
                user: std::ptr::null_mut(),
                error: err.into_raw(),
            };
        }
    };

    let request = match client.request("GET", url_str).and_then(|b| b.build().map_err(Into::into)) {
        Ok(req) => req,
        Err(e) => {
            let err = CString::new(format!("failed to build request: {}", e)).unwrap();
            return UserAuthResult {
                user: std::ptr::null_mut(),
                error: err.into_raw(),
            };
        }
    };

    let request = client.request(method, url)?.build()?;

    match client.execute(request) {
        Ok(resp) => {
            let user = UserState::new(resp); // TODO: extract public params from response
            UserAuthResult {
                user: Box::into_raw(Box::new(user)),
                error: std::ptr::null_mut(),
            }
        }
        Err(e) => {
            let err = CString::new(format!("{{\"error\":\"{}\"}}", e)).unwrap();
            UserAuthResult {
                user: std::ptr::null_mut(),
                error: err.into_raw(),
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn userauth_get_credential(user_ptr: *mut UserState, url: *const c_char) -> *mut c_char {
    let user = unsafe {
        assert!(!user_ptr.is_null());
        &mut *user_ptr
    };
    let url_str = c_char_to_string(url).unwrap_or_default();

    let mut rng = rand::thread_rng();
    let (reg_request, reg_state) = user.request(&mut rng)?;
    let request_bytes = reg_request.as_bytes();

    let client = match Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            let err = CString::new(format!("failed to build client: {}", e)).unwrap();
            err_json(err)
        }
    };

    let request = match client
    .request("POST", url_str)
    .map(|b| b.body(request_bytes.clone()))
    .and_then(|b| b.build().map_err(Into::into)) {
        Ok(req) => req,
        Err(e) => {
            let err = CString::new(format!("failed to build request: {}", e)).unwrap();
            err_json(err)
        }
    };

    match client.execute(request) {
        Ok(resp) => {
            match user.handle_response(reg_request, resp) {
                Ok(res) => {
                    if let Some(cred) = user.get_credential() {
                        // Build a JSON object from available credential attributes
                        let mut map = serde_json::Map::new();

                        if let Some(nym_id) = &cred.nym_id {
                            map.insert(
                                "nym_id".to_string(),
                                serde_json::Value::String(hex::encode(nym_id.to_bytes())),
                            );
                        }
                        if let Some(age) = &cred.age {
                            map.insert(
                                "age".to_string(),
                                serde_json::Value::Number(scalar_u32(age).unwrap().into()),
                            );
                        }
                        if let Some(mc) = &cred.measurement_count {
                            map.insert(
                                "measurement_count".to_string(),
                                serde_json::Value::Number(scalar_u32(mc).unwrap().into()),
                            );
                        }

                        let json = serde_json::Value::Object(map).to_string();
                        return CString::new(json).unwrap().into_raw();
                    } else {
                        let err = CString::new("no credential returned").unwrap();
                        err_json(err)
                    }
                }, 
                Err(e) => err_json(e)
            }
        },
        Err(e) => err_json(e)
    }

}

#[no_mangle]
pub extern "C" fn userauth_submit(user_ptr: *mut UserState, url: *const c_char, probe_cc: *const c_char, probe_asn: *const c_char) -> *mut c_char  {
    let user = unsafe {
        assert!(!user_ptr.is_null());
        &mut *user_ptr
    };
    let url_str = c_char_to_string(url).unwrap_or_default();
    let probe_cc_str = c_char_to_string(probe_cc).unwrap_or_default();
    let probe_asn_str = c_char_to_string(probe_asn).unwrap_or_default();

    let today = today(); // TODO: confirm that generating this for the client is alright since this is eventually overwritten by the server
    let age_range = (today - 30)..(today + 1);
    let measurement_count_range = 0..100;
    
    let ((submit_request, submit_state), nym) = user.submit_request(
        &mut rng,
        probe_cc.clone(),
        probe_asn.clone(),
        age_range.clone(),
        measurement_count_range.clone(),
    )?;
    let submit_request_bytes = submit_request.as_bytes();

    let client = match Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            let err = CString::new(format!("failed to build client: {}", e)).unwrap();
            err_json(err)
        }
    };

    let request = match client
    .request("POST", url_str)
    .map(|b| b.body(submit_request_bytes.clone()))
    .and_then(|b| b.build().map_err(Into::into)) {
        Ok(req) => req,
        Err(e) => {
            let err = CString::new(format!("failed to build request: {}", e)).unwrap();
            err_json(err)
        }
    };

    match client.execute(request) {
        Ok(resp) => {
            let response_bytes = resp.body_b64_bytes?;
            match user.handle_submit_response(submit_state, response_bytes) {
                Ok(res) => {
                   if let Some(cred) = user.get_credential() {
                        // Build a JSON object from available credential attributes
                        let mut map = serde_json::Map::new();

                        if let Some(nym_id) = &cred.nym_id {
                            map.insert(
                                "nym_id".to_string(),
                                serde_json::Value::String(hex::encode(nym_id.to_bytes())),
                            );
                        }
                        if let Some(age) = &cred.age {
                            map.insert(
                                "age".to_string(),
                                serde_json::Value::Number(scalar_u32(age).unwrap().into()),
                            );
                        }
                        if let Some(mc) = &cred.measurement_count {
                            map.insert(
                                "measurement_count".to_string(),
                                serde_json::Value::Number(scalar_u32(mc).unwrap().into()),
                            );
                        }

                        let json = serde_json::Value::Object(map).to_string();
                        return CString::new(json).unwrap().into_raw();
                    } else {
                        let err = CString::new("no credential returned").unwrap();
                        err_json(err)
                    } 
                },
                Err(e) => err_json(e)
            }
        }
        Err(e) => {
            err_json(e)
        }
    }
}

#[no_mangle]
pub extern "C" fn userauth_free_user(user_ptr: *mut UserState) {
    if !user_ptr.is_null() {
        unsafe {
            Box::from_raw(user_ptr); // drop happens here
        }
    }
}
