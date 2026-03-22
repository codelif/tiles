//! The main module for networking

pub mod ticket;
use std::{
    io,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use futures_util::{StreamExt, TryStreamExt};
use iroh::{
    Endpoint, EndpointId, NET_REPORT_TIMEOUT, PublicKey, SecretKey,
    address_lookup::{self, MdnsAddressLookup, mdns},
    endpoint::{BindError, presets},
    endpoint_info::UserData,
    protocol::Router,
};
use iroh_gossip::{
    Gossip, TopicId,
    api::{Event, GossipReceiver, GossipSender},
};
use iroh_ping::Ping;
use iroh_tickets::endpoint::EndpointTicket;
use rusqlite::Connection;
use tilekit::accounts::{
    get_did_from_public_key, get_random_bytes, get_random_bytes_32, get_secret_key,
};
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::core::{
    accounts::{self, get_current_user, get_user_by_user_id, save_self_account_db},
    network::ticket::{EndpointUserData, LinkTicket},
    storage::db::{DBTYPE, get_db_conn},
};
use sha2::{Digest, Sha256};

const DEVICE_LINK_LOCAL_TOPIC: &str = "com.tilesprivacy.tiles.link";
#[derive(serde::Serialize, serde::Deserialize)]
struct NetworkMessage {
    from_did: String,
    from_nickname: String,
    is_online: bool,
    body: MessageBody,
    // to prevent iroh's deduplication on same msg
    nonce: [u8; 16],
}

impl NetworkMessage {
    fn new(user: &accounts::User, is_online: bool, body: MessageBody) -> Self {
        Self {
            from_did: user.user_id.clone(),
            from_nickname: user.username.clone(),
            is_online,
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
    LinkRequest { ticket: String },
    LinkAccepted,
    LinkRejected { reason: String },
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
    let endpoint = create_endpoint(&user).await?;
    let is_online = is_online(&endpoint).await;
    let mut bootstrap_ids: Vec<EndpointId> = vec![];
    // if ticket's there, then this is link enable sender's  command, e;se receiver end
    if let Some(ticket) = ticket {
        let (endpoint_id, mut did, mut nickname, topic_value) = parse_link_ticket(&ticket)?;

        let topic_id = if is_online {
            topic_value.expect("Expected topicId")
        } else {
            create_topic_id(DEVICE_LINK_LOCAL_TOPIC)
        };

        if is_online {
            bootstrap_ids.push(endpoint_id.expect("Expected an EndpointId as bootstrapId "))
        } else {
            println!("Searching for peers in the local network..");
            let mdns = address_lookup::mdns::MdnsAddressLookup::builder().build(endpoint.id())?;
            let (new_bootstrap_ids, user_data) =
                find_offline_bootstrap_peers(&endpoint, mdns).await?;
            bootstrap_ids = new_bootstrap_ids;
            let endpoint_user_data = EndpointUserData::try_from(user_data.to_string())?;
            did = endpoint_user_data.did;
            nickname = endpoint_user_data.nickname;
        };
        if get_user_by_user_id(&user_db_conn, did.to_owned()).is_ok() {
            println!("Device {}({}) already linked", nickname, did);
            return Ok(());
        }
        let (sender, mut receiver, recv_router) =
            create_gossip_network(&endpoint, topic_id, bootstrap_ids).await?;

        println!("\nConnecting to {}({}).....", nickname, did);

        receiver.joined().await?;

        tokio::spawn(subsribe_loop(
            receiver,
            sender.clone(),
            user.clone(),
            user_db_conn,
            None,
        ));

        let link_req_msg =
            NetworkMessage::new(&user, is_online, MessageBody::LinkRequest { ticket });
        sender.broadcast(link_req_msg.to_bytes().into()).await?;

        println!("\nSent link request to {}({})", nickname, did);

        println!("\nWaiting for response...");

        tokio::signal::ctrl_c().await?;
        recv_router.shutdown().await?;
    } else {
        // RECEIVER BLOCK
        if !is_online {
            let mdns = address_lookup::mdns::MdnsAddressLookup::builder().build(endpoint.id())?;
            endpoint.address_lookup()?.add(mdns.clone());
        }

        // Its better to have unique session'ed channels while
        // when the communication is over internet
        let topic_id = if is_online {
            TopicId::from_bytes(get_random_bytes_32())
        } else {
            create_topic_id(DEVICE_LINK_LOCAL_TOPIC)
        };

        let (sender, receiver, recv_router) =
            create_gossip_network(&endpoint, topic_id, bootstrap_ids).await?;

        let generated_ticket = if is_online {
            let ticket = LinkTicket::new(
                topic_id,
                endpoint.addr(),
                user.user_id.clone(),
                user.username.clone(),
            );
            println!("Generated link ticket: \n{:?}\n", ticket.to_string());

            println!(
                "Use this ticket with `tiles link enable <ticket>` on the system you want to connect to\n"
            );
            ticket.to_string()
        } else {
            // generate a code
            let uuid = Uuid::new_v4().to_string();

            let ticket = uuid.split('-').collect::<Vec<&str>>()[0];

            println!("Generated link code: {}\n", ticket);

            println!(
                "Use this link code with `tiles link enable {}` on the system you want to connect to\n",
                ticket
            );
            ticket.to_string()
        };

        println!("Don't close this session until the link process is done\n");

        tokio::spawn(subsribe_loop(
            receiver,
            sender.clone(),
            user.clone(),
            user_db_conn,
            Some(generated_ticket),
        ));

        // TODO: Maybe a better way is to use a oneshot channel to exit
        // the terminal instead of SIGINT
        tokio::signal::ctrl_c().await?;
        recv_router.shutdown().await?;
    }
    endpoint.close().await;
    Ok(())
}

async fn subsribe_loop(
    mut receiver: GossipReceiver,
    sender: GossipSender,
    user: accounts::User,
    db_conn: Connection,
    generated_ticket: Option<String>,
) -> Result<()> {
    while let Some(event) = receiver.try_next().await? {
        if cfg!(debug_assertions) {
            println!("In {}:, some event {:?}", user.username, event);
        }
        if let Event::Received(msg) = event {
            let pub_key = msg.delivered_from;
            let msg = NetworkMessage::from_bytes(&msg.content)?;
            if !is_did_valid(&msg.from_did, pub_key)? {
                eprintln!(
                    "Incoming peer DID {} invalid, blocking request",
                    msg.from_did
                );
                continue;
            }
            match msg.body {
                MessageBody::LinkRequest { ticket } => {
                    println!(
                        "Received link request from {}({}), Do you want to link Y/N ?",
                        msg.from_nickname, msg.from_did
                    );
                    let input: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

                    let input_clone = input.clone();
                    let stdin = io::stdin();
                    spawn_blocking(move || {
                        let mut input_clone = input_clone.lock().unwrap();
                        let _ = stdin.read_line(&mut input_clone);
                    })
                    .await?;
                    let input_resp = input.lock().unwrap().trim().to_owned();

                    let link_res_resp = if input_resp.to_lowercase() == "y" {
                        if let Some(gen_ticket) = &generated_ticket
                            && !msg.is_online
                            && *gen_ticket != ticket.to_lowercase()
                        {
                            println!("\nVerifying code does not match, please try again");
                            let response = NetworkMessage::new(
                                &user,
                                msg.is_online,
                                MessageBody::LinkRejected {
                                    reason: String::from("Link code mismatch"),
                                },
                            );
                            sender.broadcast(response.to_bytes().into()).await?;
                            continue;
                        }

                        if let Err(err) =
                            save_self_account_db(&db_conn, &msg.from_did, &msg.from_nickname)
                        {
                            println!("Failed to add the peer locally due to {:?}", err);

                            continue;
                        }

                        println!(
                            "Device {}({}) is now linked\nYou can exit now by ctrl-c",
                            msg.from_nickname, msg.from_did
                        );
                        NetworkMessage::new(&user, msg.is_online, MessageBody::LinkAccepted)
                    } else {
                        println!("You can exit now by ctrl-c");
                        NetworkMessage::new(
                            &user,
                            msg.is_online,
                            MessageBody::LinkRejected {
                                reason: String::from("Peer rejected the request"),
                            },
                        )
                    };
                    input.lock().unwrap().clear();

                    sender.broadcast(link_res_resp.to_bytes().into()).await?;
                }
                MessageBody::LinkAccepted => {
                    println!("\nLink accepted by {}({})", msg.from_nickname, msg.from_did);

                    if let Err(err) =
                        save_self_account_db(&db_conn, &msg.from_did, &msg.from_nickname)
                    {
                        println!("Failed to add the peer locally due to {:?}", err);
                        return Ok(());
                    }

                    println!("\nYou can exit now by ctrl-c");

                    continue;
                }
                MessageBody::LinkRejected { reason } => {
                    println!(
                        "Oops looks like your link request has been rejected by {}({}),\nreason: {},\nexit (ctrl-c) and try again",
                        msg.from_nickname, msg.from_did, reason
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
    let usr_data = EndpointUserData::new(&user.user_id, &user.username);
    if !cfg!(debug_assertions) {
        let signing_key = get_secret_key("tiles", &user.user_id)?;
        let secret_key = SecretKey::from_bytes(&signing_key);
        Endpoint::builder(presets::N0)
            .user_data_for_address_lookup(UserData::try_from(usr_data.to_string())?)
            .secret_key(secret_key)
            .bind()
            .await
            .map_err(<BindError as Into<anyhow::Error>>::into)
    } else {
        Endpoint::builder(presets::N0)
            .user_data_for_address_lookup(UserData::try_from(usr_data.to_string())?)
            .bind()
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

async fn is_online(endpoint: &Endpoint) -> bool {
    tokio::select! {
        _ = endpoint.online() => {
            true
        }
        _ = tokio::time::sleep(Duration::from_secs(NET_REPORT_TIMEOUT)) => {
            false
        }
    }
}

// As of now we exit asap when we see a peer. This is subjected to change
// as the scale
async fn find_offline_bootstrap_peers(
    endpoint: &Endpoint,
    mdns: MdnsAddressLookup,
) -> Result<(Vec<EndpointId>, UserData)> {
    let mut bootstrap_ids: Vec<EndpointId> = vec![];
    endpoint.address_lookup()?.add(mdns.clone());
    let mut mdns_event = mdns.subscribe().await;
    let mut user_data = UserData::from_str("")?;
    while let Some(event) = mdns_event.next().await {
        match event {
            mdns::DiscoveryEvent::Discovered {
                endpoint_info,
                last_updated: _,
            } => {
                if cfg!(debug_assertions) {
                    println!("peer discoverd {:?}", endpoint_info);
                }
                bootstrap_ids.push(endpoint_info.endpoint_id);
                user_data = endpoint_info.user_data().unwrap().clone();
                break;
            }
            mdns::DiscoveryEvent::Expired { endpoint_id } => {
                if cfg!(debug_assertions) {
                    println!("peer left {:?}", endpoint_id)
                }
            }
        }
    }

    Ok((bootstrap_ids, user_data))
}

async fn create_gossip_network(
    endpoint: &Endpoint,
    topic_id: TopicId,
    bootstrap_ids: Vec<iroh::PublicKey>,
) -> Result<(GossipSender, GossipReceiver, Router)> {
    let gossip = Gossip::builder().spawn(endpoint.clone());
    let recv_router = Router::builder(endpoint.clone())
        .accept(iroh_gossip::ALPN, gossip.clone())
        .spawn();

    let (goss_sender, goss_receiver) = gossip.subscribe(topic_id, bootstrap_ids).await?.split();

    Ok((goss_sender, goss_receiver, recv_router))
}

// We handle the parsing in this way since ticket can be an encoded `LinkTicket`
// or just a 5 byte hex if linking over mDNS
fn parse_link_ticket(
    ticket: &str,
) -> Result<(Option<EndpointId>, String, String, Option<TopicId>)> {
    if let Ok(parsed_ticket) = LinkTicket::from_str(ticket) {
        Ok((
            Some(parsed_ticket.addr.id),
            parsed_ticket.did,
            parsed_ticket.nickname,
            Some(parsed_ticket.topic_id),
        ))
    } else if ticket.len() == 8 {
        // NOTE: We only have len check as a "parser" for the offline code
        // but this will surely change once we fix the code format
        Ok((None, String::from(""), String::from(""), None))
    } else {
        Err(anyhow::anyhow!("Invalid Ticket"))
    }
}

fn is_did_valid(did: &str, pub_key: PublicKey) -> Result<bool> {
    Ok(get_did_from_public_key(&pub_key)? != did)
}
// fn subsribe_mdns_events(mdns_events) {}
//TODO: Add tests, can we get some from iroh reference?
