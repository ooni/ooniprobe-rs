//! TCP connection establishment with tracing.
//!
//! [`TracingDialer`] connects to a remote endpoint and emits a
//! [`TcpConnectResult`] into the trace

use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

use crate::{
    archival::{NetworkEvent, NetworkOperation, TcpConnectResult, TcpConnectStatus},
    errors::OoniError,
    trace::Trace,
};

/// Connects TCP sockets and records the result in a [`Trace`].
pub struct TracingDialer {
    trace: Trace,
}

impl TracingDialer {
    pub fn new(trace: Trace) -> Self {
        Self { trace }
    }

    /// Dial `addr`, record the result, return a [`TracingStream`] on success.
    pub async fn connect(&self, addr: SocketAddr, tx_id: i64) -> Result<TracingStream, OoniError> {
        let t0 = self.trace.elapsed_secs();
        let result = TcpStream::connect(addr).await;
        let t = self.trace.elapsed_secs();

        let (ip, port) = (addr.ip().to_string(), addr.port());

        match result {
            Ok(stream) => {
                self.trace.record_tcp_connect(TcpConnectResult {
                    conn_id: None,
                    dial_id: None,
                    ip: ip.clone(),
                    port,
                    status: TcpConnectStatus {
                        failure: None,
                        success: true,
                        blocked: None,
                    },
                    t0,
                    t,
                    tags: Some(vec![]),
                    transaction_id: Some(tx_id),
                });

                self.trace.record_network_event(NetworkEvent {
                    address: Some(addr.to_string()),
                    conn_id: None,
                    dial_id: None,
                    failure: None,
                    num_bytes: None,
                    operation: NetworkOperation::Connect,
                    proto: Some("tcp".into()),
                    t0,
                    t,
                    tags: None,
                    transaction_id: Some(tx_id),
                });

                Ok(TracingStream::new(
                    stream,
                    addr.to_string(),
                    self.trace.clone(),
                    tx_id,
                ))
            }
            Err(e) => {
                let err = OoniError::from_io(e);
                self.trace.record_tcp_connect(TcpConnectResult {
                    conn_id: None,
                    dial_id: None,
                    ip,
                    port,
                    status: TcpConnectStatus {
                        failure: Some(err.failure().0.clone()),
                        success: false,
                        blocked: None,
                    },
                    t0,
                    t,
                    tags: Some(vec![]),
                    transaction_id: Some(tx_id),
                });
                Err(err)
            }
        }
    }

    /// Convenience: try each address in `addrs` in order, returning the first
    /// successful connection
    pub async fn connect_first(
        &self,
        addrs: &[SocketAddr],
        tx_id: i64,
    ) -> Result<TracingStream, OoniError> {
        let mut last_err = OoniError::DnsNoAnswer;
        for &addr in addrs {
            match self.connect(addr, tx_id).await {
                Ok(s) => return Ok(s),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }
}

/// A `TcpStream` wrapper that emits [`NetworkEvent`]s for every I/O operation.
#[pin_project]
pub struct TracingStream {
    #[pin]
    inner: TcpStream,
    address: String,
    trace: Trace,
    tx_id: i64,
}

impl TracingStream {
    pub(crate) fn new(inner: TcpStream, address: String, trace: Trace, tx_id: i64) -> Self {
        Self {
            inner,
            address,
            trace,
            tx_id,
        }
    }

    /// Access the underlying `TcpStream` (e.g. to hand it to a TLS layer).
    pub fn into_inner(self) -> TcpStream {
        self.inner
    }

    /// Peer address string `"ip:port"`.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl AsyncRead for TracingStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();
        let before_len = buf.filled().len();
        let t0 = this.trace.elapsed_secs();
        let result = this.inner.poll_read(cx, buf);
        let t = this.trace.elapsed_secs();
        let num_bytes = (buf.filled().len() - before_len) as i64;

        let (failure, success) = match &result {
            Poll::Ready(Err(e)) => (
                Some(
                    OoniError::from_io(std::io::Error::new(e.kind(), e.to_string()))
                        .failure()
                        .0,
                ),
                false,
            ),
            Poll::Ready(Ok(())) if num_bytes == 0 => (Some("eof_error".to_owned()), false),
            _ => (None, true),
        };

        if matches!(result, Poll::Ready(_)) {
            this.trace.record_network_event(NetworkEvent {
                conn_id: None,
                dial_id: None,
                address: Some(this.address.clone()),
                failure,
                num_bytes: if success { Some(num_bytes) } else { None },
                operation: NetworkOperation::Read,
                proto: Some("tcp".into()),
                t0,
                t,
                tags: None,
                transaction_id: Some(*this.tx_id),
            });
        }
        result
    }
}

impl AsyncWrite for TracingStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.project();
        let t0 = this.trace.elapsed_secs();
        let result = this.inner.poll_write(cx, buf);
        let t = this.trace.elapsed_secs();

        if let Poll::Ready(ref r) = result {
            let (num_bytes, failure) = match r {
                Ok(n) => (Some(*n as i64), None),
                Err(e) => (
                    None,
                    Some(
                        OoniError::from_io(std::io::Error::new(e.kind(), e.to_string()))
                            .failure()
                            .0,
                    ),
                ),
            };
            this.trace.record_network_event(NetworkEvent {
                conn_id: None,
                dial_id: None,
                address: Some(this.address.clone()),
                failure,
                num_bytes,
                operation: NetworkOperation::Write,
                proto: Some("tcp".into()),
                t0,
                t,
                tags: None,
                transaction_id: Some(*this.tx_id),
            });
        }
        result
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.project();
        let t0 = this.trace.elapsed_secs();
        let result = this.inner.poll_shutdown(cx);
        let t = this.trace.elapsed_secs();
        if matches!(result, Poll::Ready(_)) {
            this.trace.record_network_event(NetworkEvent {
                conn_id: None,
                dial_id: None,
                address: Some(this.address.clone()),
                failure: None,
                num_bytes: None,
                operation: NetworkOperation::Close,
                proto: Some("tcp".into()),
                t0,
                t,
                tags: None,
                transaction_id: Some(*this.tx_id),
            });
        }
        result
    }
}
