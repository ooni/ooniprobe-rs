use tokio::net::{TcpListener, TcpStream};
use std::future::Future;
use env_logger::Env;
use log::info;


pub async fn run<F, Fut>(name: &str, port: &str, test_helper: F) 
    where 
        F : Fn(TcpStream) -> Fut,
        Fut : Future<Output = ()>
{
    init_logging();
    let addr = format!("0.0.0.0:{}", port);
    info!("Starting {} helper in: {}", name, addr);

    let listener = TcpListener::bind(addr)
        .await
        .expect(format!("Couldn't start {} server", name).as_str());

    loop {
        let (socket, _) = listener.accept().await.expect("Could not accept new msg");
        (test_helper)(socket).await;
    }
}

pub fn init_logging() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
}