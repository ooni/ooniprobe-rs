use std::pin::Pin;
use std::task::{Context, Poll};

use boring::derive;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use std::io;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

use hyper::client::conn::http2;

use crate::dataformats::FromNetworkEvent;
use crate::{dataformats, parrot, tracing};
use tokio::net::lookup_host;
use url::Url;

#[derive(Default, Debug)]
pub struct TestKeys {
    tcp_connect: Vec<dataformats::TCPConnect>,
    requests: Vec<dataformats::HttpTransaction>,
    queries: Vec<dataformats::DnsQuery>,
    tls_handshakes: Vec<dataformats::TlsHandshake>,
}

#[derive(Debug)]
pub enum ExperimentError {
    GenericError,
}

pub struct Target {
    input: String,
}

pub struct Config {}

pub trait Experiment {
    async fn run(target: Target, config: Config) -> TestKeys;
}

#[derive(Debug)]
pub struct TcpStreamWrapper {
    inner: TcpStream,
}

impl TcpStreamWrapper {
    pub fn from_stream(inner: TcpStream) -> Self {
        Self { inner }
    }
}

impl AsyncWrite for TcpStreamWrapper {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let inner = Pin::new(&mut self.get_mut().inner);
        // TODO: record that we wrote bytes into the transport to the network events list
        println!("writing");
        inner.poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let inner = Pin::new(&mut self.get_mut().inner);
        inner.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let inner = Pin::new(&mut self.get_mut().inner);
        inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let inner = Pin::new(&mut self.get_mut().inner);
        inner.poll_shutdown(cx)
    }
}

impl AsyncRead for TcpStreamWrapper {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let inner = Pin::new(&mut self.get_mut().inner);
        println!("reading");
        inner.poll_read(cx, buf)
    }
}

pub struct WebsiteExperiment;

impl WebsiteExperiment {
    pub async fn run(target: Target) -> Result<TestKeys, ExperimentError> {
        let mut test_keys = TestKeys::default();

        let measurement_start_time = quanta::Instant::now();
        let mut tracing_collector = tracing::NetworkEventCollector::new(measurement_start_time);

        let url = Url::parse(&target.input).unwrap();
        let host = url.host_str().unwrap().to_string();
        let port = match url.scheme() {
            "http" => 80,
            "https" => 443,
            // TODO: what do we do if now match?
            _ => 80,
        };

        let addr = format!("{}:{}", host, port);

        let transaction = tracing_collector.new_transaction();
        transaction.new_network_event();

        let mut addrs = lookup_host(addr.clone()).await.unwrap();
        let socket = addrs.next().unwrap();

        let stream = TcpStream::connect(&socket).await.unwrap();
        let stream_wrapper = TcpStreamWrapper::from_stream(stream);

        let mut network_event = transaction.new_network_event();
        network_event.set_proto("tcp");
        network_event.enter();
        let mut tls_handshake = dataformats::TlsHandshake::new(addr.as_str());
        let config = parrot::make_chrome_config();
        let stream = tokio_boring::connect(config, host.as_ref(), stream_wrapper)
            .await
            .unwrap();
        network_event.exit();
        tls_handshake.add_ssl(stream.ssl());
        tls_handshake.add_network_event(&network_event);
        test_keys.tls_handshakes.push(tls_handshake);

        let io: TokioIo<tokio_boring::SslStream<TcpStreamWrapper>> = TokioIo::new(stream);
        let executor = hyper_util::rt::tokio::TokioExecutor::new();

        let (sender, conn) =
            hyper::client::conn::http2::handshake::<_, _, Full<Bytes>>(executor, io)
                .await
                .map_err(|_| ExperimentError::GenericError)?;

        tokio::task::spawn(async move {
            if let Err(e) = conn.await {
                println!("Error: {:?}", e);
            }
        });

        let req = http::Request::builder()
            .method(http::Method::GET)
            .uri(target.input)
            .version(hyper::Version::HTTP_2)
            .body(Full::default())
            .map_err(|_| ExperimentError::GenericError)?;

        let res = sender
            .clone()
            .send_request(req)
            .await
            .map_err(|_| ExperimentError::GenericError)?;

        println!("Response: {:#?}", res);

        let body = res
            .collect()
            .await
            .map_err(|_| ExperimentError::GenericError)?
            .to_bytes();

        println!("{}", String::from_utf8_lossy(&body));
        println!("test_keys: {:#?}", test_keys);
        Ok(test_keys)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_experiment() {
        WebsiteExperiment::run(Target {
            input: "https://www.google.com/humans.txt".to_string(),
        })
        .await
        .unwrap();
    }
}
