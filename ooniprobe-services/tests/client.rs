mod common;

use std::time::Duration;

use common::{closed_port_url, start_server, start_server_raw, start_server_with_delay};
use ooniprobe_services::client::{Client, ClientOptions, Error, ErrorKind};

fn client_with(options: ClientOptions) -> Client {
    Client::builder()
        .set_options(options)
        .build()
        .expect("client should build")
}

fn get(client: &Client, url: &str) -> Result<ooniprobe_services::client::Response, Error> {
    let request = client
        .request("GET", url)
        .expect("failed to build request")
        .build()
        .unwrap();
    client.execute(request)
}

#[test]
fn get_parses_status_version_and_body() {
    let server = start_server("hello-body");
    let client = Client::builder().build().unwrap();

    let resp = get(&client, &format!("{}/", server.url)).expect("GET should succeed");

    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.version, "HTTP/1.1");
    assert_eq!(resp.body_text.as_deref(), Some("hello-body"));
    assert!(resp.body_b64_bytes.is_none());
    assert_eq!(server.hits(), 1);
}

#[test]
fn non_success_status_is_returned() {
    let server = start_server_raw(
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found",
    );
    let client = Client::builder().build().unwrap();

    let resp = get(&client, &format!("{}/", server.url)).expect("a 404 is still a response");

    assert_eq!(resp.status_code, 404);
    assert_eq!(resp.body_text.as_deref(), Some("not found"));
}

#[test]
fn supported_methods_are_sent() {
    let client = Client::builder().build().unwrap();

    for method in ["GET", "POST", "PUT", "PATCH", "OPTIONS"] {
        let server = start_server("ok");
        let request = client
            .request(method, &format!("{}/", server.url))
            .expect("method should be supported")
            .build()
            .unwrap();
        client.execute(request).expect("request should succeed");

        assert!(
            server.request_line().starts_with(&format!("{method} ")),
            "expected {method} in: {}",
            server.request_line()
        );
    }
}

#[test]
fn method_is_case_insensitive() {
    let server = start_server("ok");
    let client = Client::builder().build().unwrap();

    let request = client
        .request("put", &format!("{}/", server.url))
        .expect("lowercase method should be accepted")
        .build()
        .unwrap();
    client.execute(request).expect("request should succeed");

    assert!(server.request_line().starts_with("PUT "));
}

#[test]
fn unsupported_method_is_rejected() {
    let client = Client::builder().build().unwrap();

    let err = match client.request("DELETE", "http://127.0.0.1/") {
        Ok(_) => panic!("DELETE should be rejected"),
        Err(e) => e,
    };

    assert!(matches!(err, Error::InvalidHttpMethod));
    assert_eq!(err.kind(), ErrorKind::Other);
}

#[test]
fn non_text_header_value_is_base64_encoded() {
    let server = start_server_raw(
        b"HTTP/1.1 200 OK\r\nX-Text: plain\r\nX-Bin: caf\xe9\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
    );
    let client = Client::builder().build().unwrap();

    let resp = get(&client, &format!("{}/", server.url)).unwrap();

    assert!(resp
        .headers_list_text
        .contains(&("x-text".to_string(), "plain".to_string())));
    assert!(resp
        .headers_list_b64_bytes
        .contains(&("x-bin".to_string(), "Y2Fm6Q==".to_string())));
    assert!(!resp.headers_list_text.iter().any(|(k, _)| k == "x-bin"));
}

#[test]
fn body_is_decoded_with_declared_charset() {
    let server = start_server_raw(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=iso-8859-1\r\nContent-Length: 4\r\nConnection: close\r\n\r\ncaf\xe9",
    );
    let client = Client::builder().build().unwrap();

    let resp = get(&client, &format!("{}/", server.url)).unwrap();

    assert_eq!(resp.body_text.as_deref(), Some("caf\u{e9}"));
    assert!(resp.body_b64_bytes.is_none());
}

#[test]
fn unknown_charset_falls_back_to_utf8() {
    let server = start_server_raw(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=bogus\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
    );
    let client = Client::builder().build().unwrap();

    let resp = get(&client, &format!("{}/", server.url)).unwrap();

    assert_eq!(resp.body_text.as_deref(), Some("hello"));
}

#[test]
fn binary_body_is_base64_encoded() {
    let server = start_server_raw(
        b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 3\r\nConnection: close\r\n\r\n\xff\xfe\xfd",
    );
    let client = Client::builder().build().unwrap();

    let resp = get(&client, &format!("{}/", server.url)).unwrap();

    assert!(resp.body_text.is_none());
    assert_eq!(resp.body_b64_bytes.as_deref(), Some("//79"));
}

#[test]
fn response_serializes_to_json() {
    let server = start_server("hello-body");
    let client = Client::builder().build().unwrap();

    let resp = get(&client, &format!("{}/", server.url)).unwrap();
    let json: serde_json::Value = serde_json::from_str(&resp.to_json_str().unwrap()).unwrap();

    assert_eq!(json["status_code"], 200);
    assert_eq!(json["body_text"], "hello-body");
}

#[test]
fn default_user_agent_is_ooniprobe() {
    let server = start_server("ok");
    let client = Client::builder().build().unwrap();

    get(&client, &format!("{}/", server.url)).unwrap();

    let req = server.last_request().to_lowercase();
    assert!(
        req.contains("user-agent: ooniprobe"),
        "missing default UA in: {req}"
    );
}

#[test]
fn user_agent_is_overridable() {
    let server = start_server("ok");
    let mut options = ClientOptions::new();
    options.set_user_agent(Some("my-custom-agent/1.2"));
    let client = client_with(options);

    get(&client, &format!("{}/", server.url)).unwrap();

    let req = server.last_request().to_lowercase();
    assert!(
        req.contains("user-agent: my-custom-agent/1.2"),
        "missing custom UA in: {req}"
    );
}

#[test]
fn client_without_timeout_still_works() {
    let server = start_server("ok");
    let mut options = ClientOptions::new();
    options.set_timeout(None);
    let client = client_with(options);

    let resp = get(&client, &format!("{}/", server.url)).unwrap();

    assert_eq!(resp.status_code, 200);
}

#[test]
fn valid_proxy_url_builds() {
    let mut options = ClientOptions::new();
    options.set_proxy_url(Some("socks5h://127.0.0.1:9050"));

    assert!(Client::builder().set_options(options).build().is_ok());
}

#[test]
fn invalid_proxy_url_fails_to_build() {
    let mut options = ClientOptions::new();
    options.set_proxy_url(Some("http://[::1"));

    assert!(Client::builder().set_options(options).build().is_err());
}

#[test]
fn slow_server_is_a_timeout() {
    let server = start_server_with_delay("late", Duration::from_millis(500));
    let mut options = ClientOptions::new();
    options.set_timeout(Some(0.1));
    let client = client_with(options);

    let err = match get(&client, &format!("{}/", server.url)) {
        Ok(_) => panic!("request should time out"),
        Err(e) => e,
    };

    assert_eq!(err.kind(), ErrorKind::Timeout, "got {err:?}");
}

#[test]
fn refused_connection_is_a_connection_error() {
    let client = Client::builder().build().unwrap();

    let err = match get(&client, &format!("{}/", closed_port_url())) {
        Ok(_) => panic!("nothing is listening, request should fail"),
        Err(e) => e,
    };

    assert_eq!(err.kind(), ErrorKind::Connection, "got {err:?}");
}

#[test]
fn truncated_body_is_a_response_error() {
    // Content-Length promises 100 bytes, but the server sends 10 and closes.
    let server = start_server_raw(
        b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n0123456789",
    );
    let client = Client::builder().build().unwrap();

    let err = match get(&client, &format!("{}/", server.url)) {
        Ok(_) => panic!("a truncated body should fail"),
        Err(e) => e,
    };

    assert_eq!(err.kind(), ErrorKind::Response, "got {err:?}");
}

#[test]
fn invalid_url_fails_to_build_request() {
    let client = Client::builder().build().unwrap();

    let result = client
        .request("GET", "not a url")
        .expect("method is valid")
        .build();

    let err = match result {
        Ok(_) => panic!("an invalid URL should not build"),
        Err(e) => Error::from(e),
    };
    assert_eq!(err.kind(), ErrorKind::Other, "got {err:?}");
}
