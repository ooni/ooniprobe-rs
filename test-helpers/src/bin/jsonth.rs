use std::collections::HashMap;
use std::default::Default;

use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use httparse::{EMPTY_HEADER, Status, parse_headers};
use hyper::{Request, Response, StatusCode, header};
use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use log::{error, info};
use serde::Serialize;
use test_helpers::helper_runner::{read_port, run_tcp_server};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let port = read_port("8000");
    run_tcp_server("json_helper", &port, handle_json_helper).await;
}

#[derive(Serialize, Default, Clone)]
pub struct JsonResponse {
    request_line: String,
    headers_dict: HashMap<String, Vec<String>>,
}

/**
Process the HTTP Request Line and the Request Headers and
returns them in a JSON datastructure in the order
we received them.

The returned JSON dict looks like so:

```json
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
    let response = parse_line_and_headers(&socket).await;

    // Parse headers using hyper to parse the request.
    let io = TokioIo::new(socket);
    if let Err(e) = http1::Builder::new()
        .preserve_header_case(true)
        .serve_connection(
            io,
            service_fn(move |req| send_json_response(response.clone(), req)),
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
async fn send_json_response(
    response: Result<JsonResponse, String>,
    _request: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let response = match response {
        Ok(s) => s,
        Err(e) => {
            return make_error_response(
                format!("Couldn't parse line or headers: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    log_response(&response);
    make_response(&response)
}

async fn parse_line_and_headers(socket: &TcpStream) -> Result<JsonResponse, String> {
    // Recommended size for uri is 8000 octets, longest part of the request line
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-references
    // We also add more headspace to parse headers as well
    let mut buffer = [0u8; 4 * 8192];

    // use peek to avoid consuming from the stream
    match socket.peek(&mut buffer).await {
        Ok(0) => Err("Connection closed unexpectedly".to_string()),
        Ok(n) => {
            // Parse request line
            let request_line = parse_line(&buffer[..n]).await?;

            // Start of headers is len of request line + 2 due to the \r\n terminator
            let start = request_line.len() + 2;

            // Parse headers from buffer, starting after the request line:
            let headers_dict = parse_headers_list(&buffer[start..])?;

            Ok(JsonResponse {
                request_line,
                headers_dict,
            })
        }
        Err(e) => Err(format!("Unable to read from socket: {e}")),
    }
}

async fn parse_line(buffer: &[u8]) -> Result<String, String> {
    // Parse bytes as str
    let line = match std::str::from_utf8(buffer) {
        Ok(v) => v,
        Err(e) => return Err(format!("Unable to parse request line: {e}")),
    };

    line.split("\r\n")
        .next()
        .map(|s| s.to_string())
        .ok_or("Bad http request".to_string())
}

fn parse_headers_list(buffer: &[u8]) -> Result<HashMap<String, Vec<String>>, String> {
    let mut headers_dict: HashMap<String, Vec<String>> = HashMap::new();
    let mut headers_buff = [EMPTY_HEADER; 100];

    let headers = match parse_headers(buffer, &mut headers_buff) {
        Ok(Status::Complete((_, headers))) => headers,
        Ok(Status::Partial) => {
            return Err("Buffer too small to contain headers".into());
        }
        Err(e) => return Err(e.to_string()),
    };

    // Parse header values
    for header in headers.iter() {
        let entry = headers_dict.entry(header.name.into()).or_default();

        // Note that headers are not usually utf8, but every ascii header is valid utf8.
        // We will note enforce it here, but hyper will when parsing the request
        let value = match std::str::from_utf8(header.value) {
            Ok(v) => v.to_string(),
            Err(e) => {
                return Err(format!("Error parsing header, non-utf8 header found: {e}"));
            }
        };

        entry.push(value);
    }

    Ok(headers_dict)
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
