//! The main module for networking

use anyhow::Result;
use iroh::{Endpoint, protocol::Router};
use iroh_ping::Ping;

// Entrypoint of network connection
pub async fn init() -> Result<()> {
    let endpoint = Endpoint::bind().await?;
    endpoint.online().await;

    let ping = Ping::new();

    let recv_router = Router::builder(endpoint)
        .accept(iroh_ping::ALPN, ping)
        .spawn();

    let addr = recv_router.endpoint().addr();

    println!("{:?}", addr);

    // create a send side & send a ping
    let send_ep = Endpoint::bind().await?;
    let send_pinger = Ping::new();
    let rtt = send_pinger.ping(&send_ep, addr).await?;

    println!("ping took: {:?} to complete", rtt);
    Ok(())
}
