use jsonrpsee::server::{RpcModule, Server};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_addr = run_server().await?;
    let url = format!("http://{}", server_addr);
    println!("Server running at {}", url);

    loop {}
}

async fn run_server() -> anyhow::Result<SocketAddr> {
    let server = Server::builder()
        .build("0.0.0.0:9000".parse::<SocketAddr>()?)
        .await?;
    let mut module = RpcModule::new(());
    module.register_method("insert", |params, _| {
        dbg!(params.as_str());
        dbg!(params.parse::<serde_json::Value>()).unwrap();
    })?;

    let addr = server.local_addr()?;
    let handle = server.start(module);

    // In this example we don't care about doing shutdown so let's it run forever.
    // You may use the `ServerHandle` to shut it down or manage it yourself.
    tokio::spawn(handle.stopped());

    Ok(addr)
}
