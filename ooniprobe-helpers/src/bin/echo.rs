use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use ooniprobe_helpers::helper_runner::run;
use log::{error, debug};


#[tokio::main]
async fn main() {
    run("echoth", "8000", handle_tcp_echo).await;
}

async fn handle_tcp_echo(socket: TcpStream) {
    let mut buffer = [0u8; 1024];
    let mut socket = socket;
    match socket.read(&mut buffer).await {
        Ok(n) => {
            debug!("Receiving {} bytes", n);
            if let Err(e) = socket.write_all(&buffer[0..n]).await {
                error!("Error trying to write response {}", e);
            }
        }
        Err(e) => {
            error!("Error trying to read from request: {}", e);
        }
    };
}