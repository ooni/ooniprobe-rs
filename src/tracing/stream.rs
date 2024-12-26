use std::io;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

use super::network_event::{NetworkEvent, NetworkEventTransaction};

#[derive(Debug)]
#[pin_project::pin_project]
pub struct TracingTcpStream {
    #[pin]
    inner: TcpStream,
    network_events: Vec<NetworkEvent>,
    transaction: NetworkEventTransaction,
}

impl TracingTcpStream {
    pub fn from_stream(inner: TcpStream, transaction: NetworkEventTransaction) -> Self {
        Self {
            inner,
            transaction,
            network_events: vec![],
        }
    }

    pub fn print_network_events(self) {
        println!("{:?}", self.network_events);
    }
}

impl AsyncWrite for TracingTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        let ret = this.inner.poll_write(cx, buf);
        let num_bytes = match &ret {
            Poll::Ready(Ok(n)) => *n,
            _ => 0,
        };
        let mut ne = this.transaction.new_network_event();
        ne.set_operation("write");
        ne.set_num_bytes(num_bytes.try_into().unwrap());
        this.network_events.push(ne);
        ret
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_shutdown(cx)
    }
}

impl AsyncRead for TracingTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let initial_filled = buf.filled().len();
        let this = self.project();
        let ret = this.inner.poll_read(cx, buf);
        let num_bytes = buf.filled().len() - initial_filled;
        let mut ne = this.transaction.new_network_event();
        ne.set_operation("read");
        ne.set_num_bytes(num_bytes.try_into().unwrap());
        this.network_events.push(ne);
        ret
    }
}
