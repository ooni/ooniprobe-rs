//! Offline coverage for the uniffi client_get / client_post surface.

mod common;

use std::time::Duration;

use common::{start_server, start_server_with_delay};
use uniffi_ooniprobe::{client_get, client_post, KeyValue};

fn kv(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: value.to_string(),
    }
}

#[test]
fn get_parses_status_and_body() {
    let server = start_server("hello-body");
    let resp = client_get(format!("{}/", server.url), vec![], vec![], None, None)
        .expect("GET should succeed");

    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.body_text.as_deref(), Some("hello-body"));
    assert!(resp.body_b64_bytes.is_none());
    assert_eq!(server.hits(), 1);
}

#[test]
fn get_forwards_headers() {
    let server = start_server("ok");
    client_get(
        format!("{}/", server.url),
        vec![kv("X-Test", "abc"), kv("X-Other", "xyz")],
        vec![],
        None,
        None,
    )
    .expect("GET should succeed");

    let req = server.last_request().to_lowercase();
    assert!(req.contains("x-test: abc"), "missing header in: {req}");
    assert!(req.contains("x-other: xyz"), "missing header in: {req}");
}

#[test]
fn get_appends_query_params() {
    let server = start_server("ok");
    client_get(
        format!("{}/search", server.url),
        vec![],
        vec![kv("q", "hello"), kv("lang", "en")],
        None,
        None,
    )
    .expect("GET should succeed");

    let line = server.request_line();
    assert!(
        line.contains("q=hello") && line.contains("lang=en"),
        "expected query params in: {line}"
    );
}

#[test]
fn post_forwards_body() {
    let server = start_server("ok");
    let resp = client_post(
        format!("{}/submit", server.url),
        vec![kv("Content-Type", "application/json")],
        r#"{"k":"v"}"#.to_string(),
        None,
        None,
    )
    .expect("POST should succeed");

    assert_eq!(resp.status_code, 200);
    let req = server.last_request();
    assert!(req.starts_with("POST "), "expected POST, got: {req}");
    assert!(req.contains(r#"{"k":"v"}"#), "payload missing from: {req}");
}

#[test]
fn default_timeout_allows_fast_response() {
    let server = start_server("ok");
    let resp = client_get(format!("{}/", server.url), vec![], vec![], None, None)
        .expect("fast response should succeed with default timeout");
    assert_eq!(resp.status_code, 200);
}

#[test]
fn explicit_timeout_is_enforced() {
    let server = start_server_with_delay("ok", Duration::from_millis(1500));
    let result = client_get(format!("{}/", server.url), vec![], vec![], None, Some(0.2));
    assert!(
        result.is_err(),
        "slow response should time out, got: {result:?}"
    );
}

#[test]
fn generous_timeout_tolerates_slow_response() {
    let server = start_server_with_delay("ok", Duration::from_millis(300));
    let resp = client_get(format!("{}/", server.url), vec![], vec![], None, Some(5.0))
        .expect("response within timeout should succeed");
    assert_eq!(resp.status_code, 200);
}
