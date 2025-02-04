use anyhow::Result;
use iroh::{Endpoint, NodeAddr, PublicKey, SecretKey};
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<()> {
    let secret_key = SecretKey::generate(rand::rngs::OsRng);

    let endpoint = Endpoint::builder()
        .secret_key(secret_key)
        .alpns(vec![b"ethersync".to_vec()])
        .discovery_n0()
        .bind()
        .await?;

    println!("> our node id: {}", endpoint.node_id());

    // get first command line arg
    if let Some(peer) = std::env::args().nth(1) {
        let public_key = PublicKey::from_str(&peer)?;
        let node_addr: NodeAddr = public_key.into();
        let conn = endpoint.connect(node_addr, b"ethersync").await?;
        dbg!(conn.peer_identity().unwrap());
        dbg!(conn.remote_node_id());
        let (mut send, recv) = conn.accept_bi().await?;
        loop {
            let mut s = "".to_string();
            let line = std::io::stdin().read_line(&mut s).unwrap();
            send.write_all(s.as_bytes()).await?;
            dbg!(conn.rtt());
        }
        send.finish()?;
    } else {
        loop {
            let conn = endpoint.accept().await.unwrap().await?;
            dbg!(conn.peer_identity().unwrap());
            dbg!(conn.remote_node_id());
            dbg!(conn.rtt());
            let (mut send, mut recv) = conn.open_bi().await?;
            send.write_all(b"yo").await?;
            let mut buf = [0u8; 1024];
            loop {
                if let Some(res) = recv.read(&mut buf).await? {
                    println!("Received: {:?}", std::str::from_utf8(&buf[..res])?);
                    dbg!(conn.rtt());
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}
