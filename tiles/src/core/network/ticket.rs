//! Tickets for Networking
use std::{fmt::Display, str::FromStr};

use iroh::EndpointAddr;
use iroh_gossip::TopicId;
use iroh_tickets::Ticket;

//TODO: Add tests
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct LinkTicket {
    pub nickname: String,
    pub did: String,
    pub addr: EndpointAddr,
    pub topic_id: TopicId,
}

impl Ticket for LinkTicket {
    const KIND: &'static str = "link";

    fn to_bytes(&self) -> Vec<u8> {
        postcard::to_stdvec(&self).expect("serde_json to bytes couldnt be done")
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, iroh_tickets::ParseError> {
        postcard::from_bytes(bytes).map_err(Into::into)
    }
}

impl Display for LinkTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut text = data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        write!(f, "{}", text)
    }
}

impl FromStr for LinkTicket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ticket_bytes = data_encoding::BASE32_NOPAD.decode(s.to_uppercase().as_bytes())?;
        LinkTicket::from_bytes(&ticket_bytes).map_err(Into::into)
    }
}

impl LinkTicket {
    pub fn new(topic_id: TopicId, addr: EndpointAddr, did: String, nickname: String) -> Self {
        LinkTicket {
            addr,
            topic_id,
            did,
            nickname,
        }
    }
}
