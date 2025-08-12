use std::collections::HashMap;

use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::{Request, Response, StatusCode, header};
use log::{debug, error, info};
use ooniprobe_helpers::helper_runner::{run_http_server};
use serde::Serialize;
use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;


#[tokio::main]
async fn main() {
    // run_tcp_server("json_helper", "8000", handle_json_helper).await;
    run_http_server("json_helper", "8000", handle_json_helper).await;
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
async fn handle_json_helper(request : Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>>{
    let mut resp = JsonResponse::default();
    let headers = request.headers();

    resp.request_line = "NOT IMPLEMENTED".to_string();
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