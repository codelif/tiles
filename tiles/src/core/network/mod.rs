//! The main module for networking

pub mod ticket;
use std::{io, str::FromStr};

use anyhow::Result;
use futures_util::TryStreamExt;
use iroh::{
    Endpoint, EndpointId, SecretKey,
    endpoint::{BindError, presets},
    protocol::Router,
};
use iroh_gossip::{
    Gossip, TopicId,
    api::{Event, GossipReceiver, GossipSender},
};
use iroh_ping::Ping;
use iroh_tickets::endpoint::EndpointTicket;
use rusqlite::Connection;
use tilekit::accounts::{get_did_from_public_key, get_random_bytes, get_secret_key};

use crate::core::{
    accounts::{self, get_current_user, get_user_by_user_id, save_self_account_db},
    network::ticket::LinkTicket,
    storage::db::{DBTYPE, get_db_conn},
};
use sha2::{Digest, Sha256};

#[derive(serde::Serialize, serde::Deserialize)]
struct NetworkMessage {
    body: MessageBody,

    // to prevent iroh's deduplication on same msg
    nonce: [u8; 16],
}

impl NetworkMessage {
    fn new(body: MessageBody) -> Self {
        Self {
            body,
            nonce: get_random_bytes(),
        }
    }
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        postcard::from_bytes(bytes).map_err(Into::into)
    }
    fn to_bytes(&self) -> Vec<u8> {
        postcard::to_stdvec(&self).expect("Failed to convert to bytes w postcard")
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[allow(clippy::enum_variant_names)]
enum MessageBody {
    LinkRequest { did: String, nickname: String },
    LinkAccepted { did: String, nickname: String },
    LinkRejected { did: String, nickname: String },
}

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
    let user_db_conn = get_db_conn(DBTYPE::COMMON)?;
    let user = get_current_user(&user_db_conn)?;
    if let Some(ticket) = ticket {
        let link_ticket = LinkTicket::from_str(&ticket)?;
        if get_user_by_user_id(&user_db_conn, &link_ticket.did).is_err() {
            println!(
                "Device {}({}) already linked",
                link_ticket.nickname, link_ticket.did
            );
            return Ok(());
        }
        let endpoint = create_endpoint(&user).await?;
        endpoint.online().await;
        let gossip = Gossip::builder().spawn(endpoint.clone());

        let recv_router = Router::builder(endpoint.clone())
            .accept(iroh_gossip::ALPN, gossip.clone())
            .spawn();

        let (sender, mut receiver) = gossip
            .subscribe(link_ticket.topic_id, vec![link_ticket.addr.id])
            .await?
            .split();

        println!(
            "Connecting to {}({}).....",
            link_ticket.nickname, link_ticket.did
        );
        receiver.joined().await?;
        tokio::spawn(subsribe_loop(
            receiver,
            sender.clone(),
            user.clone(),
            user_db_conn,
        ));

        let link_req_msg = NetworkMessage::new(MessageBody::LinkRequest {
            did: user.user_id,
            nickname: user.username,
        });
        sender.broadcast(link_req_msg.to_bytes().into()).await?;

        println!(
            "Sent link request to {}({})",
            link_ticket.nickname, link_ticket.did
        );

        println!("Waiting for response...");
        tokio::signal::ctrl_c().await?;
        recv_router.shutdown().await?;
        endpoint.close().await;
    } else {
        let endpoint = create_endpoint(&user).await?;
        endpoint.online().await;

        let gossip = Gossip::builder().spawn(endpoint.clone());

        let recv_router = Router::builder(endpoint.clone())
            .accept(iroh_gossip::ALPN, gossip.clone())
            .spawn();

        let topic_id = create_topic_id("com.tilesprivacy.tiles.link");

        let (sender, receiver) = gossip.subscribe(topic_id, vec![]).await?.split();

        let ticket = LinkTicket::new(
            topic_id,
            endpoint.addr(),
            user.user_id.clone(),
            user.username.clone(),
        );

        println!("Link Ticket: {:?}\n", ticket.to_string());
        println!(
            "Use this link ticket with `tiles link <ticket>` on the system you want to connect to\n"
        );

        println!("Don't close this session until the link process is done\n");

        tokio::spawn(subsribe_loop(
            receiver,
            sender.clone(),
            user.clone(),
            user_db_conn,
        ));

        // TODO: Maybe a better way is to use a oneshot channel to exit
        // the terminal instead of SIGINT
        tokio::signal::ctrl_c().await?;
        recv_router.shutdown().await?;
        endpoint.close().await;
    }
    Ok(())
}

async fn subsribe_loop(
    mut receiver: GossipReceiver,
    sender: GossipSender,
    user: accounts::User,
    db_conn: Connection,
) -> Result<()> {
    while let Some(event) = receiver.try_next().await? {
        // println!("some event {:?}", event);
        if let Event::Received(msg) = event {
            match NetworkMessage::from_bytes(&msg.content)?.body {
                MessageBody::LinkRequest { did, nickname } => {
                    println!(
                        "Received link request from {}({}), Do you want to link Y/N ?",
                        nickname, did
                    );
                    let stdin = io::stdin();
                    let mut input = String::new();
                    stdin.read_line(&mut input)?;
                    input = input.trim().to_owned();
                    let link_res_resp = if input.to_lowercase() == "y" {
                        save_self_account_db(&db_conn, &did, &nickname)?;
                        println!(
                            "Device {}({}) is now linked\nYou can exit now by ctrl-c",
                            nickname, did
                        );
                        NetworkMessage::new(MessageBody::LinkAccepted {
                            did: user.user_id.clone(),
                            nickname: user.username.clone(),
                        })
                    } else {
                        println!("You can exit now by ctrl-c");
                        NetworkMessage::new(MessageBody::LinkRejected {
                            did: user.user_id.clone(),
                            nickname: user.username.clone(),
                        })
                    };
                    input.clear();
                    sender.broadcast(link_res_resp.to_bytes().into()).await?;
                }
                MessageBody::LinkAccepted { did, nickname } => {
                    save_self_account_db(&db_conn, &did, &nickname)?;
                    println!("Link accepted by {}({})", nickname, did);

                    println!("You can exit now by ctrl-c");

                    return Ok(());
                }
                MessageBody::LinkRejected { did, nickname } => {
                    println!(
                        "Oops looks like your link request has been rejected by {}({}), exit (ctrl-c) and try again",
                        nickname, did
                    );
                }
            }
        }
    }
    Ok(())
}

async fn create_endpoint(user: &accounts::User) -> Result<Endpoint> {
    // In release mode, we will build the endpoint using
    // tiles keypair in keychain
    if !cfg!(debug_assertions) {
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

fn create_topic_id(topic_name: &str) -> TopicId {
    let mut hasher = Sha256::new();
    hasher.update(topic_name.as_bytes());
    let topic_id_bytes = hasher.finalize();
    TopicId::from_bytes(topic_id_bytes.into())
}

fn _get_did_from_endpoint(endpoint_id: EndpointId) -> Result<String> {
    get_did_from_public_key(endpoint_id.as_bytes())
}

//TODO: Add tests, can we get some from iroh reference?
