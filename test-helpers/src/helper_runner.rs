use http_body_util::combinators::BoxBody;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};

use anyhow::Result;
use bytes::Bytes;
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, TcpStream};

use env_logger::Env;
use log::{error, info};

use std::env;
use std::future::Future;

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

    serve_tcp(listener, test_helper).await
}

/// Accepts connections on `listener` forever, handling each one concurrently
/// with `test_helper`.
pub async fn serve_tcp<Fut>(listener: TcpListener, test_helper: fn(TcpStream) -> Fut)
where
    Fut: Future<Output = ()> + Send + 'static,
{
    loop {
        let (socket, _) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                error!("Could not accept new msg: {e}");
                continue;
            }
        };

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

    serve_http(listener, handler).await
}

/// Serves HTTP/1 on `listener` forever, answering every request with `handler`.
pub async fn serve_http<F, Fut>(listener: TcpListener, handler: F)
where
    F: Fn(Request<hyper::body::Incoming>) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = Result<Response<BoxBody<Bytes, hyper::Error>>>>
        + Send
        + 'static,
{
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                error!("Could not accept new msg: {e}");
                continue;
            }
        };

        let io = TokioIo::new(stream);
        let handler = handler.clone();

        tokio::task::spawn(async move {
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
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or("info")).try_init();
}

pub fn read_port(default: &str) -> String {
    let args: Vec<String> = env::args().collect();
    parse_port(&args, default)
}

/// Returns the value after `--port` in `args`, or `default` if there is none.
pub fn parse_port(args: &[String], default: &str) -> String {
    if let Some(i) = args.iter().position(|s| s == "--port") {
        args.get(i + 1)
            .expect("Missing argument for --port. Usage: --port <port_num>")
            .clone()
    } else {
        default.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::{BodyExt, Empty, Full};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_port_reads_value_after_flag() {
        let port = parse_port(&args(&["ooechohelpd", "--port", "9000"]), "8000");
        assert_eq!(port, "9000");
    }

    #[test]
    fn parse_port_without_flag_uses_default() {
        let port = parse_port(&args(&["ooechohelpd"]), "8000");
        assert_eq!(port, "8000");
    }

    #[test]
    #[should_panic(expected = "Missing argument for --port")]
    fn parse_port_without_value_panics() {
        parse_port(&args(&["ooechohelpd", "--port"]), "8000");
    }

    #[test]
    fn read_port_without_flag_uses_default() {
        // The test runner's own arguments don't include --port.
        assert_eq!(read_port("8000"), "8000");
    }

    #[test]
    fn init_logging_can_be_called_twice() {
        init_logging();
        init_logging();
    }

    async fn uppercase_once(mut stream: TcpStream) {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        stream.write_all(&buf.to_ascii_uppercase()).await.unwrap();
    }

    #[tokio::test]
    async fn serve_tcp_handles_each_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(serve_tcp(listener, uppercase_once));

        let mut first = TcpStream::connect(addr).await.unwrap();
        let mut second = TcpStream::connect(addr).await.unwrap();

        for (client, msg) in [(&mut first, "first"), (&mut second, "second")] {
            client.write_all(msg.as_bytes()).await.unwrap();
            client.shutdown().await.unwrap();
        }

        let mut received = String::new();
        first.read_to_string(&mut received).await.unwrap();
        assert_eq!(received, "FIRST");

        received.clear();
        second.read_to_string(&mut received).await.unwrap();
        assert_eq!(received, "SECOND");
    }

    async fn hello_handler(
        _req: Request<hyper::body::Incoming>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>> {
        let body = Full::new(Bytes::from_static(b"hello"))
            .map_err(|never| match never {})
            .boxed();
        Ok(Response::new(body))
    }

    #[tokio::test]
    async fn serve_http_answers_requests() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(serve_http(listener, hello_handler));

        let stream = TcpStream::connect(addr).await.unwrap();
        let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .unwrap();
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                error!("Connection failed: {e}");
            }
        });

        let request = Request::builder()
            .uri("/")
            .header("Host", "example.com")
            .body(Empty::<Bytes>::new())
            .unwrap();
        let response = sender.send_request(request).await.unwrap();
        assert_eq!(response.status(), hyper::StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"hello");
    }
}
