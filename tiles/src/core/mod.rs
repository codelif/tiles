//! tiles-core
//!
//! The core runtime which different UI apps can leverage
//! Generally the core will be run as daemon and interact with other sub components

use anyhow::Result;

use crate::core::{
    accounts::save_root_account_db,
    storage::db::{Dbconn, init_db},
};

pub mod accounts;
pub mod chats;
pub mod health;
pub mod network;
pub mod storage;

// Entrypoint of the core
pub fn init(db_conn: &Dbconn) -> Result<()> {
    save_root_account_db(db_conn)
}
