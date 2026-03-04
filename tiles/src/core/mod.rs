//! tiles-core
//!
//! The core runtime which different UI apps can leverage
//! Generally the core will be run as daemon and interact with other sub components

use anyhow::Result;

use crate::core::{accounts::save_root_account_db, storage::db::init_db};

pub mod accounts;
pub mod chats;
pub mod health;
pub mod storage;
// Entrypoint of the core
pub fn init() -> Result<()> {
    init_db()?;
    save_root_account_db()
}
