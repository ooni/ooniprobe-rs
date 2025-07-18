use env_logger::Env;
use log::info;
use std::future::Future;
use tokio::net::{TcpListener, TcpStream};

pub async fn run<Fut>(name: &str, port: &str, test_helper: fn(TcpStream) -> Fut)
where
    Fut: Future<Output = ()> + Send + 'static,
{
    init_logging();
    let addr = format!("0.0.0.0:{port}");
    info!("Starting {name} helper in: {addr}");

    let listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Couldn't start {name} server: {e}"));

    loop {
        let (socket, _) = listener
            .accept()
            .await
            .unwrap_or_else(|e| panic!("Could not accept new msg: {e}"));
        tokio::spawn(async move {
            // Process each socket concurrently.
            (test_helper)(socket).await
        });
    }
}

pub fn init_logging() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
}
