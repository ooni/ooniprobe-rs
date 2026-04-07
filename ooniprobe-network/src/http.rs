//! HTTP transport with tracing.
//!
//! [`TracingHttpClient`] sends HTTP/1.1 or HTTP/2 requests and records
//! the full request/response cycle as an [`HttpTransaction`]

use std::collections::HashMap;

use bytes::Bytes;
use http::{Method, Request, Response, Uri};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    archival::{HttpRequest, HttpResponse, HttpTransaction, MaybeBinaryData},
    errors::OoniError,
    trace::Trace,
};

const MAX_BODY: usize = 524_288;

/// collect_headers
fn collect_headers(map: &http::HeaderMap) -> (Vec<(String, String)>, HashMap<String, String>) {
    let list: Vec<(String, String)> = map
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_owned()))
        .collect();
    let map: HashMap<String, String> = list.iter().cloned().collect();
    (list, map)
}

/// Request metadata captured before the body is consumed.
struct RequestMeta {
    url: String,
    method: String,
    headers_list: Vec<(String, String)>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl RequestMeta {
    fn capture(req: &Request<Full<Bytes>>) -> Self {
        let url = req.uri().to_string();
        let method = req.method().to_string();
        let (hdrs_list, hdrs) = collect_headers(req.headers());
        
        let body = req
            .body()
            .clone()
            .collect()
            .now_or_never()
            .and_then(|r| r.ok())
            .map(|c| c.to_bytes().to_vec())
            .unwrap_or_default();
        Self {
            url,
            method,
            headers_list: hdrs_list,
            headers: hdrs,
            body,
        }
    }

    fn to_http_request(&self) -> HttpRequest {
        HttpRequest {
            body: MaybeBinaryData(self.body.clone()),
            body_is_truncated: false,
            headers_list: self.headers_list.clone(),
            headers: self.headers.clone(),
            method: self.method.clone(),
            url: self.url.clone(),
            x_transport: "tcp".into(),
        }
    }
}

// TracingHttpClient
pub struct TracingHttpClient {
    trace: Trace,
    max_body_size: usize,
}

impl TracingHttpClient {
    pub fn new(trace: Trace) -> Self {
        Self {
            trace,
            max_body_size: MAX_BODY,
        }
    }

    pub fn with_max_body_size(mut self, n: usize) -> Self {
        self.max_body_size = n;
        self
    }

    pub async fn send_http1<S>(
        &self,
        stream: S,
        request: Request<Full<Bytes>>,
        address: &str,
        alpn: &str,
        tx_id: i64,
    ) -> Result<Response<Incoming>, OoniError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let meta = RequestMeta::capture(&request);
        let io = TokioIo::new(stream);
        let t0 = self.trace.elapsed_secs();

        let send_result = hyper::client::conn::http1::handshake(io)
            .await
            .map_err(|_| OoniError::HttpRequestFailed)
            .and_then(|(mut sender, conn)| {
                tokio::spawn(async move {
                    let _ = conn.await;
                });
                Ok(sender)
            });

        match send_result {
            Err(e) => {
                self.record_failure(&meta, address, alpn, tx_id, t0, &e);
                Err(e)
            }
            Ok(mut sender) => {
                let resp = sender
                    .send_request(request)
                    .await
                    .map_err(|_| OoniError::HttpRequestFailed);
                let t = self.trace.elapsed_secs();
                self.record_result(meta, address, alpn, tx_id, t0, t, resp)
            }
        }
    }

    pub async fn send_http2<S>(
        &self,
        stream: S,
        request: Request<Full<Bytes>>,
        address: &str,
        alpn: &str,
        tx_id: i64,
    ) -> Result<Response<Incoming>, OoniError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let meta = RequestMeta::capture(&request);
        let io = TokioIo::new(stream);
        let exec = hyper_util::rt::TokioExecutor::new();
        let t0 = self.trace.elapsed_secs();

        let send_result = hyper::client::conn::http2::handshake::<_, _, Full<Bytes>>(exec, io)
            .await
            .map_err(|_| OoniError::HttpRequestFailed)
            .and_then(|(mut sender, conn)| {
                tokio::spawn(async move {
                    let _ = conn.await;
                });
                Ok(sender)
            });

        match send_result {
            Err(e) => {
                self.record_failure(&meta, address, alpn, tx_id, t0, &e);
                Err(e)
            }
            Ok(mut sender) => {
                let resp = sender
                    .send_request(request)
                    .await
                    .map_err(|_| OoniError::HttpRequestFailed);
                let t = self.trace.elapsed_secs();
                self.record_result(meta, address, alpn, tx_id, t0, t, resp)
            }
        }
    }

    fn record_failure(
        &self,
        meta: &RequestMeta,
        address: &str,
        alpn: &str,
        tx_id: i64,
        t0: f64,
        err: &OoniError,
    ) {
        let t = self.trace.elapsed_secs();
        self.trace.record_http_request(HttpTransaction {
            network: Some("tcp".into()),
            address: Some(address.to_owned()),
            alpn: Some(alpn.to_owned()),
            failure: Some(err.failure().0.clone()),
            request: meta.to_http_request(),
            response: HttpResponse::default(),
            t0,
            t,
            tags: None,
            transaction_id: Some(tx_id),
        });
    }

    fn record_result(
        &self,
        meta: RequestMeta,
        address: &str,
        alpn: &str,
        tx_id: i64,
        t0: f64,
        t: f64,
        result: Result<Response<Incoming>, OoniError>,
    ) -> Result<Response<Incoming>, OoniError> {
        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let (hdrs_list, hdrs) = collect_headers(resp.headers());
                self.trace.record_http_request(HttpTransaction {
                    network: "tcp".into(),
                    address: address.to_owned(),
                    alpn: alpn.to_owned(),
                    failure: None,
                    request: meta.to_http_request(),
                    response: HttpResponse {
                        body: MaybeBinaryBody(vec![]),
                        body_is_truncated: false,
                        code: status,
                        headers_list: hdrs_list,
                        headers: hdrs,
                    },
                    t0,
                    t,
                    tags: vec![],
                    transaction_id: Some(tx_id),
                });
                Ok(resp)
            }
            Err(e) => {
                self.record_failure(&meta, address, alpn, tx_id, t0, &e);
                Err(e)
            }
        }
    }
}


// Trait to call `collect()` synchronously on `Full<Bytes>` (it's always ready).
trait NowOrNever: std::future::Future + Sized {
    fn now_or_never(self) -> Option<Self::Output> {
        use std::pin::Pin;
        use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        fn noop(_: *const ()) {}
        fn noop_clone(_: *const ()) -> RawWaker {
            make_waker()
        }
        fn make_waker() -> RawWaker {
            static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        let waker = unsafe { Waker::from_raw(make_waker()) };
        let mut cx = Context::from_waker(&waker);
        match Pin::new(&mut { self }).poll(&mut cx) {
            Poll::Ready(v) => Some(v),
            Poll::Pending => None,
        }
    }
}
impl<F: std::future::Future> NowOrNever for F {}

/// Build a simple GET request with OONI's default User-Agent.
pub fn get_request(url: &str) -> Result<Request<Full<Bytes>>, OoniError> {
    let uri: Uri = url
        .parse()
        .map_err(|e| OoniError::Unknown(format!("invalid URL: {e}")))?;
    let host = uri
        .host()
        .ok_or_else(|| OoniError::Unknown("URL has no host".into()))?
        .to_owned();

    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("Host", &host)
        .header("User-Agent", ooni_user_agent())
        .header("Accept", "*/*")
        .header("Accept-Language", "en-US;q=0.8,en;q=0.5")
        .body(Full::new(Bytes::new()))
        .map_err(|e| OoniError::Unknown(format!("request build error: {e}")))
}

/// OONI probe User-Agent string.
pub fn ooni_user_agent() -> &'static str {
    "ooniprobe-rs/0.1.0"
}

/// Collect and optionally truncate the body of a response.
pub async fn read_body(response: Response<Incoming>, max_bytes: usize) -> (Vec<u8>, bool) {
    match response.into_body().collect().await {
        Ok(c) => {
            let b = c.to_bytes();
            if b.len() > max_bytes {
                (b[..max_bytes].to_vec(), true)
            } else {
                (b.to_vec(), false)
            }
        }
        Err(_) => (vec![], false),
    }
}
