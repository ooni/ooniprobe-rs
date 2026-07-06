use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct MockServer {
    pub url: String,
    hits: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl MockServer {
    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    pub fn last_request(&self) -> String {
        self.requests().last().cloned().unwrap_or_default()
    }

    pub fn request_line(&self) -> String {
        self.last_request().lines().next().unwrap_or("").to_string()
    }
}

fn content_length(headers: &str) -> usize {
    for line in headers.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                return v.trim().parse().unwrap_or(0);
            }
        }
    }
    0
}

pub fn start_server_with_delay(body: &'static str, delay: Duration) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");

    let hits = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let hits_t = Arc::clone(&hits);
    let requests_t = Arc::clone(&requests);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };

            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            let head_end = loop {
                match stream.read(&mut buf) {
                    Ok(0) => break None,
                    Ok(n) => {
                        data.extend_from_slice(&buf[..n]);
                        if let Some(p) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                            break Some(p + 4);
                        }
                    }
                    Err(_) => break None,
                }
            };
            let Some(head_end) = head_end else { continue };

            let headers = String::from_utf8_lossy(&data[..head_end]).to_string();
            let expected = head_end + content_length(&headers);
            while data.len() < expected {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => data.extend_from_slice(&buf[..n]),
                }
            }

            requests_t
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&data).to_string());

            hits_t.fetch_add(1, Ordering::SeqCst);

            if !delay.is_zero() {
                thread::sleep(delay);
            }

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });

    MockServer {
        url: format!("http://{addr}"),
        hits,
        requests,
    }
}

pub fn start_server(body: &'static str) -> MockServer {
    start_server_with_delay(body, Duration::ZERO)
}
