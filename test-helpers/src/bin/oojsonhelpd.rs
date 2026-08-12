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

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::{BodyExt, Empty};

    #[tokio::test]
    async fn parse_line_extracts_request_line() {
        let buf = b"GET /path HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let line = parse_line(buf).await.unwrap();
        assert_eq!(line, "GET /path HTTP/1.1");
    }

    #[tokio::test]
    async fn parse_line_without_crlf_returns_whole_buffer() {
        let buf = b"GET /path HTTP/1.1";
        let line = parse_line(buf).await.unwrap();
        assert_eq!(line, "GET /path HTTP/1.1");
    }

    #[tokio::test]
    async fn parse_line_rejects_non_utf8() {
        let buf = [0xff, 0xfe, 0xfd];
        let result = parse_line(&buf).await;
        assert!(result.is_err());
    }

    #[test]
    fn parse_headers_list_collects_single_values() {
        let buf = b"Host: example.com\r\nAccept: text/plain\r\n\r\n";
        let headers = parse_headers_list(buf).unwrap();
        assert_eq!(headers.get("Host"), Some(&vec!["example.com".to_string()]));
        assert_eq!(
            headers.get("Accept"),
            Some(&vec!["text/plain".to_string()])
        );
    }

    #[test]
    fn parse_headers_list_collects_repeated_header_names() {
        let buf = b"Accept: application/json\r\nAccept: text/plain\r\n\r\n";
        let headers = parse_headers_list(buf).unwrap();
        assert_eq!(
            headers.get("Accept"),
            Some(&vec![
                "application/json".to_string(),
                "text/plain".to_string()
            ])
        );
    }

    #[test]
    fn parse_headers_list_with_no_headers_returns_empty_map() {
        let buf = b"\r\n";
        let headers = parse_headers_list(buf).unwrap();
        assert!(headers.is_empty());
    }

    #[test]
    fn parse_headers_list_partial_buffer_is_error() {
        // No terminating blank line, so httparse can't tell if headers are complete.
        let buf = b"Host: example.com\r\n";
        let result = parse_headers_list(buf);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn make_response_serializes_json_body() {
        let response = JsonResponse {
            request_line: "GET / HTTP/1.1".to_string(),
            headers_dict: HashMap::from([("Host".to_string(), vec!["example.com".to_string()])]),
        };

        let resp = make_response(&response).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["request_line"], "GET / HTTP/1.1");
        assert_eq!(parsed["headers_dict"]["Host"][0], "example.com");
    }

    #[tokio::test]
    async fn make_error_response_serializes_message() {
        let resp =
            make_error_response("boom".to_string(), StatusCode::INTERNAL_SERVER_ERROR).unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["message"], "boom");
    }

    #[tokio::test]
    async fn test_jsonth_basic() {
        let mut sender = spawn_json_server(true).await;

        let request = Request::builder()
            .uri("/some/path")
            .header("Host", "example.com")
            .header("Accept", "application/json")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["request_line"], "GET /some/path HTTP/1.1");
        assert_eq!(parsed["headers_dict"]["Host"][0], "example.com");
        assert_eq!(parsed["headers_dict"]["Accept"][0], "application/json");
    }

    #[tokio::test]
    /// Test that we can detect when a header is changed in its way to the
    /// server
    async fn test_jsonth_header_cases() {
        let mut sender = spawn_json_server(false).await;

        let request = Request::builder()
            .uri("/some/path")
            .header("Host", "example.com")
            .header("Accept", "application/json")
            .body(Empty::<Bytes>::new())
            .unwrap();

        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["request_line"], "GET /some/path HTTP/1.1");
        assert_eq!(parsed["headers_dict"]["host"][0], "example.com");
        assert_eq!(parsed["headers_dict"]["accept"][0], "application/json");
    }

    /// Starts the json helper on an ephemeral port and returns a client
    /// sender already connected to it.
    ///
    /// Note that it uses the title_case_headers from hyper to check if the
    /// server is not rewriting the casing of headers. This is the kind of thing
    /// we want to detect
    async fn spawn_json_server(title_case_headers : bool) -> hyper::client::conn::http1::SendRequest<Empty<Bytes>> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_json_helper(stream).await;
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let io = TokioIo::new(stream);

        let (sender, conn) = hyper::client::conn::http1::Builder::new()
            .title_case_headers(title_case_headers)
            .handshake(io)
            .await
            .unwrap();
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                error!("Connection failed: {e}");
            }
        });

        sender
    }
}
