//! Offline coverage for the uniffi userauth surface.

mod common;

use std::time::Duration;

use common::{start_server, start_server_with_delay};
use uniffi_ooniprobe::{get_probe_id, userauth_register, userauth_submit, OoniError};

const PUBLIC_PARAMS: &str = "AdqzxWc0xFMFlXygX+KfKxRGy6EEOgukeGokXmfsBA0QAUiqSrbV636keUJkvV8SfGpuD3P1sqor6w6jlTZxUIN6AwAAAAAAAADK2ygnqfhicm2pXO8Tu73Pu4AhHrJExfG1rW8uLk1UfQzxKzdpwnhmUx7qsdD9yXoy3J1B4Bh4OXMan2VfTPJVvs7JmVFr3V6iSqgoV1+RJfgQZXq5WB9439tng+4bUWs=";
const MANIFEST_VERSION: &str = "TjxIhQyJHRZsqmidU_coSEl2dZUiBGvL";

#[test]
fn register_rejects_invalid_public_params() {
    let result = userauth_register(
        "http://example.invalid/".to_string(),
        "not-valid-base64".to_string(),
        MANIFEST_VERSION.to_string(),
        None,
        None,
    );
    assert!(result.is_err(), "invalid public params should error");
}

#[test]
fn register_posts_to_server() {
    let server = start_server("{}");
    let _ = userauth_register(
        format!("{}/sign_credential", server.url),
        PUBLIC_PARAMS.to_string(),
        MANIFEST_VERSION.to_string(),
        None,
        None,
    );
    assert_eq!(server.hits(), 1);
    assert!(server.request_line().starts_with("POST "));
}

#[test]
fn submit_without_credential_succeeds() {
    let server = start_server("{}");
    let result = userauth_submit(
        format!("{}/submit_measurement", server.url),
        "{}".to_string(),
        "IT".to_string(),
        "AS117".to_string(),
        None,
        None,
        None,
    )
    .expect("uncredentialed submit should succeed");

    assert!(result.credential.is_none());
    assert_eq!(result.response.status_code, 200);
    assert_eq!(server.hits(), 1);
    assert!(server.request_line().starts_with("POST "));
}

#[test]
fn submit_routes_through_proxy() {
    let origin = start_server("{}");
    let proxy = start_server("{}");
    let result = userauth_submit(
        format!("{}/submit_measurement", origin.url),
        "{}".to_string(),
        "IT".to_string(),
        "AS117".to_string(),
        Some(proxy.url.clone()),
        None,
        None,
    )
    .expect("uncredentialed submit should succeed");

    assert_eq!(result.response.status_code, 200);
    assert_eq!(proxy.hits(), 1, "submit should route through the proxy");
    assert_eq!(origin.hits(), 0, "origin should not be contacted directly");
}

#[test]
fn submit_enforces_timeout() {
    let server = start_server_with_delay("{}", Duration::from_millis(1500));
    let result = userauth_submit(
        format!("{}/submit_measurement", server.url),
        "{}".to_string(),
        "IT".to_string(),
        "AS117".to_string(),
        None,
        Some(0.2),
        None,
    );
    assert!(
        matches!(result, Err(OoniError::TimeoutError(_))),
        "slow submit should be a TimeoutError, got: {result:?}"
    );
}

#[test]
fn get_probe_id_rejects_invalid_credential() {
    let result = get_probe_id(
        "not-valid-base64".to_string(),
        "AS117".to_string(),
        "IT".to_string(),
    );
    assert!(result.is_err(), "invalid credential should error");
}
