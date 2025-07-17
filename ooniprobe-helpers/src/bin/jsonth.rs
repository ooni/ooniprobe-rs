use std::collections::HashMap;

use log::{debug, error, info};
use ooniprobe_helpers::helper_runner::run;
use serde::Serialize;
use serde_json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const MAX_HEADER_LINE_LEN: usize = 16384;
const MAX_HEADERS: usize = 500;

#[tokio::main]
async fn main() {
    run("json_helper", "8000", handle_json_helper).await;
}

#[derive(Serialize)]
pub struct Response {
    request_line: String,
    request_headers: Vec<Vec<String>>,
    headers_dict: HashMap<String, Vec<String>>,
}

impl Response {
    pub fn new() -> Response {
        Response {
            request_line: String::new(),
            request_headers: Vec::new(),
            headers_dict: HashMap::new(),
        }
    }
}

/*
This is a simplified version of http to overcome
header lowercase normalization. It does not actually implement the HTTP
protocol, but only the subset of it that we need for testing.

What this HTTP channel currently does is process the HTTP Request Line and
the Request Headers and returns them in a JSON datastructure in the order
we received them.

The returned JSON dict looks like so:

{
'request_headers':
[['User-Agent', 'IE6'], ['Content-Length', 200]]
'request_line':
'GET / HTTP/1.1'
}
*/
async fn handle_json_helper(socket: TcpStream) {
    let mut socket = socket;
    let reader = BufReader::new(&mut socket);
    let mut resp = Response::new();

    // Read request line
    let mut lines = reader.lines();
    match lines.next_line().await {
        Ok(None) => {
            error!("Connection closed by client");
            return;
        }
        Err(e) => {
            error!("Error reading request: {}", e);
            return;
        }
        Ok(Some(l)) => {
            resp.request_line = l.trim().to_string();
        }
    }

    // Read headers
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if resp.request_headers.len() >= MAX_HEADERS {
                    error!("Maximum number of headers received.");
                    return_error(socket, 400, "too_many_headers").await;
                    return;
                }

                if line.is_empty() {
                    // End of headers
                    break;
                }

                if line.len() > MAX_HEADER_LINE_LEN {
                    return_error(socket, 400, "max_header_length_exceeeded").await;
                    return;
                }

                // For multiline headers
                let line_bytes = line.as_bytes();
                if line_bytes[0] == b' ' || line_bytes[0] == b'\t' {
                    // This is to support header field value folding over multiple lines
                    // as specified by rfc2616.
                    match resp.request_headers.last_mut() {
                        None => {
                            return_error(socket, 400, "malformed_header").await;
                            return;
                        }
                        Some(h) => {
                            h[1] = format!("{}\n{}", h[1], line);
                        }
                    }
                    continue;
                }

                // For regular headers
                match line.split_once(":") {
                    Some((key, val)) => {
                        let key = key.trim();
                        let val = val.trim();
                        resp.request_headers
                            .push(vec![key.to_string(), val.to_string()]);
                    }
                    None => error!("Got malformed HTTP Header request field: {}", line),
                }
            }
            Ok(None) => {
                error!("Connection closed by client");
                return;
            }
            Err(e) => panic!("Could not read headers: {}", e),
        }
    }

    // Fill headers dict
    for header in &resp.request_headers {
        let key = &header[0];
        let val = &header[1];
        resp.headers_dict
            .entry(key.clone())
            .or_insert_with(Vec::new)
            .push(val.clone());
    }
    log_response(&resp);

    // Write response back
    let body = match serde_json::to_string(&resp) {
        Ok(s) => s,
        Err(e) => {
            panic!("Unable to serialize response object. Error: {}", e);
        }
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type:application/json\r\n\r\n{}",
        body.len(),
        body
    );

    match socket.write_all(response.as_bytes()).await {
        Ok(_) => debug!("Response sent successfully"),
        Err(e) => error!("Couldn't write response back: {}", e),
    }
}

fn log_response(resp: &Response) {
    // request line, user agent, host
    let mut user_agent = "<not provided>";
    for header in &resp.request_headers {
        if header[0].to_lowercase() == "user-agent" {
            user_agent = header[1].as_str();
            break;
        }
    }

    let mut host = "<not provided>";
    for header in &resp.request_headers {
        if header[0].to_lowercase() == "host" {
            host = header[1].as_str();
            break;
        }
    }

    info!(
        "{} - User-Agent: {} - Host: {}",
        resp.request_line, user_agent, host
    );
}

async fn return_error(mut socket: TcpStream, error_code: u32, error_msg: &str) {
    let response = format!("HTTP/1.1 {} {}", error_code, error_msg);
    error!("{}", response);
    if let Err(e) = socket.write_all(response.as_bytes()).await {
        panic!("Could not send response: {}", e);
    }
}
