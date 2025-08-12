use http_body_util::combinators::BoxBody;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};

use anyhow::Result;
use bytes::Bytes;
use hyper_util::rt::TokioIo;

use env_logger::Env;
use log::{error, info};
use std::future::Future;
use tokio::net::{TcpListener, TcpStream};

pub async fn run_tcp_server<Fut>(name: &str, port: &str, test_helper: fn(TcpStream) -> Fut)
where
    Fut: Future<Output = ()> + Send + 'static,
{
    init_logging();
    let addr = format!("0.0.0.0:{port}");
    info!("Starting {name} helper in: {addr}");

    let listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Couldn't start {name} server: {e}"));

    loop {
        let (socket, _) = listener
            .accept()
            .await
            .unwrap_or_else(|e| panic!("Could not accept new msg: {e}"));
        tokio::spawn(async move {
            // Process each socket concurrently.
            (test_helper)(socket).await
        });
    }
}

pub async fn run_http_server<F, Fut>(name: &str, socket: &str, handler: F)
where
    F: Fn(Request<hyper::body::Incoming>) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = Result<Response<BoxBody<Bytes, hyper::Error>>>>
        + Send
        + 'static,
{
    init_logging();
    info!("Starting {name} server...");

    let addr = format!("0.0.0.0:{socket}");
    let listener = TcpListener::bind(addr.clone())
        .await
        .unwrap_or_else(|e| panic!("Couldn't start {name} server: {e}"));

    info!("Listening on http://{addr}");

    loop {
        let (stream, _) = listener
            .accept()
            .await
            .unwrap_or_else(|e| panic!("Could not accept new msg: {e}"));

        let io = TokioIo::new(stream);
        let handler = handler.clone();

        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .serve_connection(io, service_fn(handler))
                .await
            {
                error!("Error serving connection: {err:?}");
            }
        });
    }
}

pub fn init_logging() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
}
