//! The main module for networking

use std::{any::Any, str::FromStr};

use anyhow::Result;
use iroh::{
    Endpoint, EndpointId, PublicKey, SecretKey,
    endpoint::{BindError, presets},
    protocol::Router,
};
use iroh_ping::Ping;
use iroh_tickets::endpoint::EndpointTicket;
use tilekit::accounts::get_secret_key;

use crate::core::{
    accounts::get_current_user,
    storage::db::{DBTYPE, get_db_conn},
};

// Entrypoint of network connection
pub async fn init(ticket: Option<&str>) -> Result<()> {
    if let Some(ticket_addr) = ticket {
        let sender_endpoint = Endpoint::bind(presets::N0).await?;
        println!("{:?}", sender_endpoint.addr());
        let se_clone = sender_endpoint.clone();
        let send_pinger = Ping::new();
        let rtt = send_pinger
            .ping(
                &sender_endpoint,
                EndpointTicket::from_str(ticket_addr)?
                    .endpoint_addr()
                    .clone(),
            )
            .await?;

        println!("ping took: {:?} to complete", rtt);
        se_clone.close().await;
    } else {
        let endpoint = Endpoint::bind(presets::N0).await?;
        let ep = endpoint.clone();
        let ep2 = endpoint.clone();
        endpoint.online().await;

        let ping = Ping::new();

        let ticket = EndpointTicket::new(endpoint.addr());

        println!("ticket\n{:?}", ticket.to_string());

        let recv_router = Router::builder(ep).accept(iroh_ping::ALPN, ping).spawn();
        ep2.close().await;
        recv_router.shutdown().await?;
    }
    Ok(())
}

pub async fn link(ticket: Option<String>) -> Result<()> {
    let ping = Ping::new();
    if let Some(ticket) = ticket {
        let endpoint = create_endpoint(false).await?;
        endpoint.online().await;
        let endpoint_close = endpoint.clone();
        let rtt = ping
            .ping(
                &endpoint,
                EndpointTicket::from_str(&ticket)?.endpoint_addr().clone(),
            )
            .await?;

        println!("ping took: {:?} to complete", rtt);
        endpoint_close.close().await;
    } else {
        let endpoint = create_endpoint(true).await?;
        endpoint.online().await;
        let endpoint_close = endpoint.clone();
        let ticket = EndpointTicket::new(endpoint.addr());

        println!("ticket\n{:?}", ticket.to_string());
        let recv_router = Router::builder(endpoint)
            .accept(iroh_ping::ALPN, ping)
            .spawn();

        tokio::signal::ctrl_c().await?;
        recv_router.shutdown().await?;
        endpoint_close.close().await;
    }
    Ok(())
}

async fn create_endpoint(use_app_key: bool) -> Result<Endpoint> {
    if use_app_key {
        let user_db_conn = get_db_conn(DBTYPE::COMMON)?;
        let user = get_current_user(&user_db_conn)?;

        let signing_key = get_secret_key("tiles", &user.user_id)?;

        let secret_key = SecretKey::from_bytes(&signing_key);

        Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            .bind()
            .await
            .map_err(<BindError as Into<anyhow::Error>>::into)
    } else {
        Endpoint::bind(presets::N0)
            .await
            .map_err(<BindError as Into<anyhow::Error>>::into)
    }
}
