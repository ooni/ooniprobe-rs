//! Integration tests for proxy support on the uniffi-exported client surface.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use uniffi_ooniprobe::{client_get, client_post};

/// A minimal in-process HTTP server
struct MockServer {
    url: String,
    hits: Arc<AtomicUsize>,
    request_lines: Arc<Mutex<Vec<String>>>,
}

impl MockServer {
    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    fn request_lines(&self) -> Vec<String> {
        self.request_lines.lock().unwrap().clone()
    }
}

fn handle_conn(
    mut stream: TcpStream,
    body: &str,
    hits: &Arc<AtomicUsize>,
    lines: &Arc<Mutex<Vec<String>>>,
) {
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

    let text = String::from_utf8_lossy(&data);
    let request_line = text.lines().next().unwrap_or("").to_string();
    lines.lock().unwrap().push(request_line);
    hits.fetch_add(1, Ordering::SeqCst);

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Start a mock HTTP server that always responds `200 OK` with `body`.
fn start_server(body: &'static str) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");

    let hits = Arc::new(AtomicUsize::new(0));
    let request_lines = Arc::new(Mutex::new(Vec::new()));

    let hits_thread = Arc::clone(&hits);
    let lines_thread = Arc::clone(&request_lines);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            handle_conn(stream, body, &hits_thread, &lines_thread);
        }
    });

    MockServer {
        url: format!("http://{addr}"),
        hits,
        request_lines,
    }
}

#[test]
fn client_get_routes_through_proxy() {
    let origin = start_server("origin-body");
    let proxy = start_server("proxy-body");

    let resp = client_get(
        format!("{}/path", origin.url),
        vec![],
        vec![],
        Some(proxy.url.clone()),
    )
    .expect("GET through proxy should succeed");

    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.body_text.as_deref(), Some("proxy-body"));
    assert_eq!(proxy.hits(), 1, "proxy should have been hit exactly once");
    assert_eq!(origin.hits(), 0, "origin should not be contacted directly");

    let line = &proxy.request_lines()[0];
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
    )
    .expect("POST through proxy should succeed");

    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.body_text.as_deref(), Some("proxy-body"));
    assert_eq!(proxy.hits(), 1, "proxy should have been hit exactly once");
    assert_eq!(origin.hits(), 0, "origin should not be contacted directly");

    let line = &proxy.request_lines()[0];
    assert!(
        line.starts_with("POST ") && line.contains("http://"),
        "expected absolute-form POST request line, got: {line}"
    );
}

#[test]
fn no_proxy_connects_directly() {
    let origin = start_server("origin-body");
    let proxy = start_server("proxy-body");

    let resp = client_get(format!("{}/path", origin.url), vec![], vec![], None)
        .expect("direct GET should succeed");

    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.body_text.as_deref(), Some("origin-body"));
    assert_eq!(origin.hits(), 1, "origin should be contacted directly");
    assert_eq!(proxy.hits(), 0, "proxy must not be used when proxy is None");
}

#[test]
fn invalid_proxy_url_is_rejected() {
    // An unsupported/malformed proxy scheme should fail while building the client.
    let result = client_get(
        "http://example.invalid/".to_string(),
        vec![],
        vec![],
        Some("://missing-scheme".to_string()),
    );
    assert!(result.is_err(), "malformed proxy URL should error, got: {result:?}");
}

#[test]
fn dead_proxy_yields_connection_error() {
    let result = client_get(
        "http://example.invalid/".to_string(),
        vec![],
        vec![],
        Some("http://127.0.0.1:1".to_string()),
    );
    assert!(result.is_err(), "dead proxy should error, got: {result:?}");
}
