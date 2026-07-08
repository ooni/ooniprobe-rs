//! Integration tests for proxy support on the uniffi-exported client surface.

mod common;

use common::start_server;
use uniffi_ooniprobe::{client_get, client_post, OoniError};

#[test]
fn client_get_routes_through_proxy() {
    let origin = start_server("origin-body");
    let proxy = start_server("proxy-body");

    let resp = client_get(
        format!("{}/path", origin.url),
        vec![],
        vec![],
        Some(proxy.url.clone()),
        None,
        None,
    )
    .expect("GET through proxy should succeed");

    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.body_text.as_deref(), Some("proxy-body"));
    assert_eq!(proxy.hits(), 1, "proxy should have been hit exactly once");
    assert_eq!(origin.hits(), 0, "origin should not be contacted directly");

    let line = proxy.request_line();
    assert!(
        line.starts_with("GET ") && line.contains("http://"),
        "expected absolute-form GET request line, got: {line}"
    );
}

#[test]
fn client_post_routes_through_proxy() {
    let origin = start_server("origin-body");
    let proxy = start_server("proxy-body");

    let resp = client_post(
        format!("{}/submit", origin.url),
        vec![],
        "hello".to_string(),
        Some(proxy.url.clone()),
        None,
        None,
    )
    .expect("POST through proxy should succeed");

    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.body_text.as_deref(), Some("proxy-body"));
    assert_eq!(proxy.hits(), 1, "proxy should have been hit exactly once");
    assert_eq!(origin.hits(), 0, "origin should not be contacted directly");

    let line = proxy.request_line();
    assert!(
        line.starts_with("POST ") && line.contains("http://"),
        "expected absolute-form POST request line, got: {line}"
    );
}

#[test]
fn no_proxy_connects_directly() {
    let origin = start_server("origin-body");
    let proxy = start_server("proxy-body");

    let resp = client_get(format!("{}/path", origin.url), vec![], vec![], None, None, None)
        .expect("direct GET should succeed");

    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.body_text.as_deref(), Some("origin-body"));
    assert_eq!(origin.hits(), 1, "origin should be contacted directly");
    assert_eq!(proxy.hits(), 0, "proxy must not be used when proxy is None");
}

#[test]
fn invalid_proxy_url_is_rejected() {
    let result = client_get(
        "http://example.invalid/".to_string(),
        vec![],
        vec![],
        Some("://missing-scheme".to_string()),
        None,
        None,
    );
    assert!(
        result.is_err(),
        "malformed proxy URL should error, got: {result:?}"
    );
}

#[test]
fn dead_proxy_yields_connection_error() {
    let result = client_get(
        "http://example.invalid/".to_string(),
        vec![],
        vec![],
        Some("http://127.0.0.1:1".to_string()),
        None,
        None,
    );
    assert!(
        matches!(result, Err(OoniError::ConnectionError(_))),
        "dead proxy should be a ConnectionError, got: {result:?}"
    );
}
