use std::collections::HashMap;

use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{header, Request, Response, StatusCode};
use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use log::{error, info};
use ooniprobe_helpers::helper_runner::run_tcp_server;
use serde::Serialize;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    run_tcp_server("json_helper", "8000", handle_json_helper).await;
}

#[derive(Serialize, Default)]
pub struct JsonResponse {
    request_line: String,
    headers_dict: HashMap<String, Vec<String>>,
}

/**
Process the HTTP Request Line and the Request Headers and
returns them in a JSON datastructure in the order
we received them.

The returned JSON dict looks like so:

```
{
'request_line':
'GET / HTTP/1.1',
'headers_dict' : {'Accept': ['application/json', 'text/plain']}
}
```
*/
async fn handle_json_helper(socket: TcpStream) {
    // Note that hyper can't give us the request line, so we parse it before
    // going to hyper
    let request_line = parse_request_line(&socket).await;

    if let Err(e) = request_line {
        error!("Couldn't parse request line: {e}");
        return;
    };

    // Parse headers using hyper to parse the request.
    let io = TokioIo::new(socket);
    if let Err(e) = http1::Builder::new()
        .preserve_header_case(true)
        .serve_connection(
            io,
            service_fn(move |req| handle_json_helper_headers(request_line.clone(), req)),
        )
        .await
    {
        error!("Could not serve request: {e}")
    }
}

/**
   Parse headers and send response using hyper

   hyper can't give you the request line, so we parse the request line manually
   before calling this handler
*/
async fn handle_json_helper_headers(
    request_line: Result<String, String>,
    request: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let request_line = match request_line {
        Ok(s) => s,
        Err(e) => {
            return make_error_response(
                format!("Couldn't parse request line: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };
    let headers = request.headers();
    let mut resp = JsonResponse {
        request_line,
        ..Default::default()
    };

    for (header, value) in headers.iter() {
        let header_list = resp.headers_dict.entry(header.to_string()).or_default();

        match value.to_str() {
            Ok(s) => header_list.push(s.to_string()),
            Err(e) => {
                let msg = format!("Unexpected non-ascii header: {e}");
                error!("{msg}");
                return make_error_response(msg, StatusCode::BAD_REQUEST);
            }
        }
    }

    log_response(&resp);
    make_response(&resp)
}

async fn parse_request_line(socket: &TcpStream) -> Result<String, String> {
    // Recommended size for uri is 8000 octets, longest part of the request line
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-references
    let mut buffer = [0u8; 8192];

    // use peek to avoid consuming from the stream
    match socket.peek(&mut buffer).await {
        Ok(0) => Err("Connection closed unexpectedly".to_string()),
        Ok(n) => {
            // Parse bytes as str
            let line = match std::str::from_utf8(&buffer[..n]) {
                Ok(v) => v,
                Err(e) => return Err(format!("Unable to parse request line: {e}")),
            };

            // Parse only request line
            line.split("\r\n")
                .next()
                .map(|s| s.to_string())
                .ok_or("Bad http request".to_string())
        }
        Err(e) => Err(format!("Unable to read from socket: {e}")),
    }
}

fn make_response(resp: &JsonResponse) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let json = serde_json::to_vec(&resp).expect("Couldn't serialize response");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap())
}

#[derive(Serialize)]
pub struct ErrorResponse {
    message: String,
}

fn make_error_response(
    message: String,
    status: StatusCode,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let resp = ErrorResponse { message };
    let json = serde_json::to_vec(&resp).expect("Couldn't serialize response");

    Ok(Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(json)))
        .unwrap())
}

fn log_response(resp: &JsonResponse) {
    // request line, user agent, host
    let mut user_agent = "<not provided>";
    for (key, value) in &resp.headers_dict {
        if key.to_lowercase() == "user-agent" {
            user_agent = value[0].as_str();
            break;
        }
    }

    let mut host = "<not provided>";
    for (key, value) in &resp.headers_dict {
        if key.to_lowercase() == "host" {
            host = value[0].as_str();
            break;
        }
    }

    info!(
        "{} - User-Agent: {} - Host: {}",
        resp.request_line, user_agent, host
    );
}
