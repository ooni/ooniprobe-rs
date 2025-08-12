use std::collections::HashMap;

use hyper::{server::conn::http1, service::service_fn};
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::{Request, Response, StatusCode, header};
use hyper_util::rt::TokioIo;
use log::{info, error};
use ooniprobe_helpers::helper_runner::run_tcp_server;
use serde::Serialize;
use anyhow::{anyhow, Result};
use bytes::Bytes;
use http_body_util::Full;
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
'GET / HTTP/1.1',
'headers_dict' : {'Accept': ['application/json', 'text/plain']}
}
*/

async fn handle_json_helper(socket : TcpStream) {

    // Note that hyper can't give us the request line, so we parse it before
    // going to hyper
    let request_line = match parse_request_line(&socket).await {
        Ok(v) => v,
        Err(e) => {
            error!("Couldn't parse request line: {e}");
            return;
        }
    };

    // Parse headers using hyper to parse the request. 
    let io = TokioIo::new(socket);
    if let Err(e) = http1::Builder::new()
                        .preserve_header_case(true)
                        .serve_connection(
                            io,  service_fn(
                                |req| 
                                handle_json_helper_headers(request_line.clone(), req)
                            )
                        ).await 
    {
        error!("Could not serve request: {e}")
    }

}

/**
    Parse headers and send response using hyper

    hyper can't give you the request line, so we parse the request line manually
    before calling this handler
 */
async fn handle_json_helper_headers(request_line: String, request : Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>>{
    let headers = request.headers();
    let mut resp = JsonResponse::default();
    resp.request_line = request_line;

    for (header, value ) in headers.iter() {
        let header_list = resp
            .headers_dict
            .entry(header.to_string())
            .or_insert_with(Vec::new);

        header_list
            .push(
                value
                .to_str()
                .expect("Unexpected non-ascii header")
                .to_string()
            );
    }

    log_response(&resp);
    return make_response(&resp)
}

async fn parse_request_line(socket : &TcpStream) -> Result<String> {
    // Recommended size for uri is 8000 octets, longest part of the request line
    // https://www.rfc-editor.org/rfc/rfc9110.html#name-uri-references
    let mut buffer = [0u8; 8192];
    
    // use peek to avoid consuming from the stream
    match socket.peek(&mut buffer).await {
        Ok(0) => {
            return Err(anyhow!("Connection closed unexpectedly"));
        }
        Ok(n) => {
            // Parse bytes as str
            let line = std::str::from_utf8(&buffer[..n])
                .map_or_else(
                    |e| panic!("Unable to parse request line: {e}"),
                    |x| x
                );
            
            // Parse only request line
            Ok(line
                .split("\r\n")
                .next()
                .expect("Bad http request")
                .to_string())
        }
        Err(e) => panic!("Unable to read from socket: {e}")
    }
}

fn make_response(resp : &JsonResponse) -> Result<Response<BoxBody<Bytes, hyper::Error>>>{
    let json = serde_json::to_vec(&resp).expect("Couldn't serialize response");
    let body = Full::from(Bytes::from(json))
                                        // map unfallible to a hyper error. 
                                        // Since unfallible will never occur, use anyerror
                                        .map_err(|_| unreachable!()) 
                                        .boxed();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
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
    for (key, value) in &resp.headers_dict{
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