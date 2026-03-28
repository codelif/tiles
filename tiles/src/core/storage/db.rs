//! Core Database Handling
//!
//! Uses sqlite as the underlying database
//!

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use rusqlite::Connection;

use crate::utils::config::{ConfigProvider, DefaultProvider};
use rusqlite_migration::{M, Migrations};
pub enum DBTYPE {
    COMMON,
    CHAT,
}

pub struct Dbconn {
    pub chat: Connection,
    pub common: Connection,
}

// DEFINE MIGRATIONS

// TODO: add the schema doc
const COMMON_MIGRATION_ARRAY: &[M] = &[M::up(
    "
    CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        username TEXT NOT NULL,
        active_profile INTEGER NOT NULL DEFAULT 0 CHECK (active_profile IN (0,1)),
        account_type TEXT NOT NULL,
        root INTEGER NOT NULL DEFAULT 0 CHECK (root IN (0,1)),
        created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        UNIQUE(account_type, user_id)
    );
    ",
)];

const COMMON_MIGRATIONS: Migrations = Migrations::from_slice(COMMON_MIGRATION_ARRAY);

// TODO: add the schema doc
const CHATS_MIGRATION_ARRAY: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS chats (
        id TEXT PRIMARY KEY,
        content TEXT NOT NULL,
        resp_id TEXT,
        role TEXT NOT NULL,
        user_id TEXT NOT NULL,
        context_id TEXT,
        created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
    )",
    ),
    // After creating row_counter, we backfill the row_counter for existing rows
    // which doesnt have any
    M::up(
        "
        ALTER TABLE CHATS ADD COLUMN row_counter INTEGER;
        UPDATE chats SET row_counter = (
            SELECT rn FROM (
                SELECT id, ROW_NUMBER() OVER ( PARTITION BY user_id ORDER BY id ) as rn FROM chats
            ) t WHERE t.id = chats.id );

        ALTER TABLE CHATS ADD COLUMN session_id TEXT;
        ",
    ),
];

const CHATS_MIGRATIONS: Migrations = Migrations::from_slice(CHATS_MIGRATION_ARRAY);

pub fn init_db() -> Result<Dbconn> {
    let mut chat_conn = get_db_conn(DBTYPE::CHAT)?;
    let mut common_conn = get_db_conn(DBTYPE::COMMON)?;

    apply_migrations(&mut common_conn, &mut chat_conn)?;

    Ok(Dbconn {
        chat: chat_conn,
        common: common_conn,
    })
}

pub fn get_db_conn(db_type: DBTYPE) -> Result<Connection> {
    let db_path = get_db_path(db_type)?;
    let conn = Connection::open(db_path)
        .map_err(|e| anyhow!("Failed to create db connection due to {:?}", e))?;

    conn.pragma_update(None, "journal_mode", "WAL")?;
    Ok(conn)
}

fn apply_migrations(common_conn: &mut Connection, chat_conn: &mut Connection) -> Result<()> {
    COMMON_MIGRATIONS
        .to_latest(common_conn)
        .map_err(<rusqlite_migration::Error as Into<anyhow::Error>>::into)?;
    CHATS_MIGRATIONS.to_latest(chat_conn).map_err(|e| e.into())
}
fn get_db_path(db_type: DBTYPE) -> Result<PathBuf> {
    let user_data_dir = DefaultProvider.get_user_data_dir()?;
    match db_type {
        DBTYPE::COMMON => Ok(user_data_dir.join("common.db")),
        DBTYPE::CHAT => Ok(user_data_dir.join("chats.db")),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn migrations_test() {
        assert!(COMMON_MIGRATIONS.validate().is_ok());
        assert!(CHATS_MIGRATIONS.validate().is_ok());
    }
}
