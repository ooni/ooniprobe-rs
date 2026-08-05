use log::{error, info};
use test_helpers::helper_runner::{read_port, run_tcp_server};
use tokio::{io, net::TcpStream};

#[tokio::main]
async fn main() {
    let port = read_port("8000");
    run_tcp_server("echoth", &port, handle_tcp_echo).await;
}

async fn handle_tcp_echo(mut stream: TcpStream) {
    // For development, an easy way to test this function is starting the server and using telnet
    // to send some data and check if the data comes back properly
    //
    // ```
    // telnet localhost 8000
    // ```
    info!("Connection received");

    let (mut reader, mut writer) = stream.split();

    // Note that this function will get stucked here until the client closes the connection,
    // continuosly sending the data it receives. This is expected.
    let result = io::copy(&mut reader, &mut writer).await;
    match result {
        Ok(0) => info!("Connection closed"),
        Ok(n) => info!("Received {n} bytes in total"),
        Err(e) => error!("Error processing request: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn echoes_back_data_sent_by_the_client() {
        let addr = spawn_echo_server().await;

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"hello, echo!").await.unwrap();
        client.shutdown().await.unwrap();

        let mut received = Vec::new();
        client.read_to_end(&mut received).await.unwrap();

        assert_eq!(received, b"hello, echo!");
    }

    #[tokio::test]
    async fn echoes_back_multiple_writes() {
        let addr = spawn_echo_server().await;

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"first ").await.unwrap();
        client.write_all(b"second").await.unwrap();
        client.shutdown().await.unwrap();

        let mut received = Vec::new();
        client.read_to_end(&mut received).await.unwrap();

        assert_eq!(received, b"first second");
    }

    /// Starts the echo handler on an ephemeral port and returns its address.
    async fn spawn_echo_server() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_tcp_echo(stream).await;
        });

        addr
    }
}
