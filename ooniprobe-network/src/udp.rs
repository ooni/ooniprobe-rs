//! UDP socket with tracing.
//!
//! [`TracingUdpSocket`] wraps `tokio::net::UdpSocket` and records every
//! `send`/`recv` as a [`NetworkEvent`] in the trace, matching the same
//! style as [`TracingStream`] in `tcp.rs`.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::UdpSocket;

use crate::{
    archival::{NetworkEvent, NetworkOperation},
    errors::OoniError,
    trace::Trace,
};

/// A UDP socket that records every send/recv as a [`NetworkEvent`].
pub struct TracingUdpSocket {
    inner: UdpSocket,
    address: String,
    trace: Trace,
}

impl TracingUdpSocket {
    pub async fn connect(server: SocketAddr, trace: Trace) -> Result<Self, OoniError> {
        let bind_addr = if server.is_ipv6() {
            ":::0"
        } else {
            "0.0.0.0:0"
        };
        let sock = UdpSocket::bind(bind_addr)
            .await
            .map_err(OoniError::from_io)?;

        sock.connect(server).await.map_err(OoniError::from_io)?;
        Ok(Self {
            inner: sock,
            address: server.to_string(),
            trace,
        })
    }

    /// Send `data`, wait for a response (bounded by `timeout`), return raw bytes.
    pub async fn exchange(
        &self,
        data: &[u8],
        timeout: Duration,
        tx_id: i64,
    ) -> Result<Vec<u8>, OoniError> {
        self.record_send(data, tx_id).await?;
        tokio::time::timeout(timeout, self.record_recv(tx_id))
            .await
            .map_err(|_| OoniError::GenericTimeout)?
    }

    async fn record_send(&self, data: &[u8], tx_id: i64) -> Result<(), OoniError> {
        let t0 = self.trace.elapsed_secs();
        let res = self.inner.send(data).await;
        let t = self.trace.elapsed_secs();

        let (num_bytes, failure) = match &res {
            Ok(n) => (Some(*n as i64), None), // usize → i64: safe since UDP payload < 65535 bytes
            Err(e) => (
                None,
                Some(
                    OoniError::from_io(std::io::Error::new(e.kind(), e.to_string()))
                        .failure()
                        .0,
                ),
            ),
        };

        self.trace.record_network_event(NetworkEvent {
            conn_id: None,
            dial_id: None,
            address: Some(self.address.clone()),
            failure: failure,
            num_bytes: num_bytes,
            operation: NetworkOperation::Write,
            proto: Some("udp".into()),
            t0,
            t,
            tags: None,
            transaction_id: Some(tx_id),
        });

        res.map(|_| ()).map_err(OoniError::from_io)
    }

    async fn record_recv(&self, tx_id: i64) -> Result<Vec<u8>, OoniError> {
        let t0 = self.trace.elapsed_secs();
        let mut buf = vec![0u8; 4096];
        let res = self.inner.recv(&mut buf).await;
        let t = self.trace.elapsed_secs();

        let (num_bytes, failure) = match &res {
            Ok(n) => (Some(*n as i64), None), // usize → i64: safe since UDP payload < 65535 bytes
            Err(e) => (
                None,
                Some(
                    OoniError::from_io(std::io::Error::new(e.kind(), e.to_string()))
                        .failure()
                        .0,
                ),
            ),
        };

        self.trace.record_network_event(NetworkEvent {
            conn_id: None,
            dial_id: None,
            address: Some(self.address.clone()),
            failure: failure,
            num_bytes: num_bytes,
            operation: NetworkOperation::Read,
            proto: Some("udp".into()),
            t0,
            t,
            tags: None,
            transaction_id: Some(tx_id),
        });

        res.map(|n| {
            buf.truncate(n);
            buf
        })
        .map_err(OoniError::from_io)
    }
}
