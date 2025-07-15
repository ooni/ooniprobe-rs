use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use ooniprobe_helpers::helper_runner::run;


#[tokio::main]
async fn main() {
    run("echoth", "8000", handle_tcp_echo).await;
}

async fn handle_tcp_echo(socket: TcpStream) {
    let mut buffer = [0u8; 1024];
    let mut socket = socket;
    match socket.read(&mut buffer).await {
        Ok(n) => {
            println!("Connection received. {} bytes read", n);
            if let Err(e) = socket.write_all(&buffer[0..n]).await {
                eprintln!("Error trying to write response {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error trying to read from request: {}", e);
        }
    };
}