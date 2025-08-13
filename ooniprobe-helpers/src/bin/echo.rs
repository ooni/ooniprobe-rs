use log::{error, info};
use ooniprobe_helpers::helper_runner::run_tcp_server;
use tokio::{io, net::TcpStream};

#[tokio::main]
async fn main() {
    run_tcp_server("echoth", "8000", handle_tcp_echo).await;
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
