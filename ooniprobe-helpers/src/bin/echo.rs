use log::{debug, error, info};
use ooniprobe_helpers::helper_runner::run_tcp_server;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::main]
async fn main() {
    run_tcp_server("echoth", "8000", handle_tcp_echo).await;
}

async fn handle_tcp_echo(socket: TcpStream) {
    let mut buffer = [0u8; 4069];
    let mut socket = socket;
    info!("Connection received");
    loop {
        match socket.read(&mut buffer).await {
            Ok(0) => {
                info!("Connection closed");
                break;
            }
            Ok(n) => {
                debug!("Receiving {n} bytes");
                if let Err(e) = socket.write_all(&buffer[0..n]).await {
                    error!("Error trying to write response {e}");
                }
            }
            Err(e) => {
                error!("Error trying to read from request: {e}");
            }
        };
    }
}

