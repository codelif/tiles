//! Accounts
// Stuff related to account and identity system
use anyhow::{Result, anyhow};
use rusqlite::{Connection, types::FromSqlError};
use std::{
    fmt::Display,
    time::{SystemTime, UNIX_EPOCH},
};
use tilekit::accounts::create_identity;
use toml::Table;
use uuid::Uuid;

use crate::{
    core::storage::db::{DBTYPE, get_db_conn},
    utils::config::{get_or_create_config, save_config},
};
const ROOT_USER_CONFIG_KEY: &str = "root-user";

const ROOT_PARSE_ERROR: &str = "Failed to parse root user config";
#[allow(dead_code)]
pub struct RootUser {
    pub id: String,
    pub nickname: String,
}

#[derive(Debug)]
pub enum ACCOUNT {
    LOCAL,
}

#[derive(Debug)]
pub struct AccountError {
    pub error: String,
}

impl Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}
impl std::error::Error for AccountError {}
impl TryFrom<String> for ACCOUNT {
    type Error = AccountError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value_lower = value.to_lowercase();
        match value_lower.as_str() {
            "local" => Ok(ACCOUNT::LOCAL),
            _ => Err(AccountError {
                error: "Invalid account type".to_owned(),
            }),
        }
    }
}
impl Display for ACCOUNT {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LOCAL => write!(f, "{}", String::from("local")),
        }
    }
}

//TODO: add doc, mirrors user table schema
#[allow(dead_code)]
#[derive(Debug)]
pub struct User {
    pub id: uuid::Uuid,
    pub user_id: String,
    pub username: String,
    pub active_profile: bool,
    pub account_type: ACCOUNT,
    pub root: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Display for RootUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}\nnickname: {}\n", self.id, self.nickname)
    }
}

impl RootUser {
    pub fn new(config: &Table) -> Result<Self> {
        let id = config
            .get("id")
            .ok_or_else(|| anyhow!("Missing ID"))?
            .as_str()
            .ok_or_else(|| anyhow!("ID not a string"))?;
        let nickname = config
            .get("nickname")
            .ok_or_else(|| anyhow!("Missing Nickname"))?
            .as_str()
            .ok_or_else(|| anyhow!("Nickname not a string"))?;
        Ok(RootUser {
            id: id.to_owned(),
            nickname: nickname.to_owned(),
        })
    }

    pub fn to_table(&self) -> Table {
        let mut root_user_table = Table::new();
        root_user_table.insert(String::from("id"), toml::Value::String(self.id.clone()));
        root_user_table.insert(
            String::from("nickname"),
            toml::Value::String(self.nickname.clone()),
        );
        root_user_table
    }
}

/// Returns a `RootUser`, which represents a root user
///
/// # Params
///
/// - config: A `Table` type of entire config.toml file
pub fn get_root_user_details(config: &Table) -> Result<RootUser> {
    let root_user = config
        .get(ROOT_USER_CONFIG_KEY)
        .ok_or_else(|| anyhow!(ROOT_PARSE_ERROR))?;
    let root_user_table = root_user
        .as_table()
        .ok_or_else(|| anyhow!("root user not a table"))?;
    RootUser::new(root_user_table)
}

/// Create a root account
/// Stores the private credentials in OS secure password manager
///
/// # Params
///
/// - config: A `Table` type of entire config.toml file
/// - nickname: Nickname for the identity (Optional)
///
/// Returns the root_user_config as a `Table` type
pub fn create_root_account(config: &Table, nickname: Option<String>) -> Result<Table> {
    let root_user = config
        .get(ROOT_USER_CONFIG_KEY)
        .ok_or_else(|| anyhow!("{} doesn't exist", ROOT_USER_CONFIG_KEY))?;
    let root_user_table = root_user
        .as_table()
        .ok_or_else(|| anyhow!(ROOT_PARSE_ERROR))?;
    let root_user_data = RootUser::new(root_user_table)?;
    let did = root_user_data.id;
    if did.is_empty() {
        Ok(create_root_user(root_user_table, nickname)?)
    } else {
        Ok(root_user_table.clone())
    }
}

/// Save the root config in `Table` type to config.toml
///
/// # Params
///
/// - config: A `Table` type of entire config.toml file
/// - root_user_config: A `Table` type of root user
pub fn save_root_account(mut config: Table, root_user_config: &Table) -> Result<()> {
    config.insert(
        String::from(ROOT_USER_CONFIG_KEY),
        toml::Value::Table(root_user_config.clone()),
    );
    save_config(&config)
}

/// Sets nickname for the root account
///
/// # Params
///
/// - config: A `Table` type of entire config.toml file
/// - nickname: Nickname for the identity
///
/// Returns the root_user_config as a `Table` type
pub fn set_nickname(config: &Table, nickname: &str) -> Result<Table> {
    let root_user = config
        .get(ROOT_USER_CONFIG_KEY)
        .ok_or_else(|| anyhow!("{} doesn't exist", ROOT_USER_CONFIG_KEY))?;

    let mut root_user_table = root_user
        .as_table()
        .ok_or_else(|| anyhow!(ROOT_PARSE_ERROR))?
        .clone();
    let did = root_user_table
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or(anyhow!("Failed to get id from config"))?;
    if did.is_empty() {
        Err(anyhow::anyhow!("No Root user available"))
    } else {
        root_user_table.insert("id".to_owned(), toml::Value::String(did.to_owned()));
        root_user_table.insert(
            "nickname".to_owned(),
            toml::Value::String(nickname.to_owned()),
        );
        Ok(root_user_table)
    }
}

pub fn get_current_user(conn: &Connection) -> Result<User> {
    let mut fetch_current_user = conn.prepare("select id, user_id, username, account_type, active_profile, root, created_at, updated_at  from users where active_profile= true")?;

    fetch_current_user
        .query_one([], |row| {
            let id: String = row.get(0)?;
            let account_type: String = row.get(3)?;
            let created_at: f64 = row.get(6)?;
            let updated_at: f64 = row.get(7)?;
            Ok(User {
                id: Uuid::try_parse(&id).map_err(FromSqlError::other)?,
                user_id: row.get(1)?,
                username: row.get(2)?,
                account_type: ACCOUNT::try_from(account_type).map_err(FromSqlError::other)?,
                active_profile: row.get(4)?,
                root: row.get(5)?,

                created_at: created_at as u64,
                updated_at: updated_at as u64,
            })
        })
        .map_err(<rusqlite::Error as Into<anyhow::Error>>::into)
}
// TODO: when we support multiple accounts
// make sure that there can't be multiple rows with
// root true.
pub fn save_root_account_db() -> Result<()> {
    let conn = get_db_conn(DBTYPE::COMMON)?;
    let config = get_or_create_config()?;
    let root_user = get_root_user_details(&config)?;
    let user = User {
        id: Uuid::now_v7(),
        user_id: root_user.id,
        username: root_user.nickname,
        account_type: ACCOUNT::LOCAL,
        active_profile: true,
        root: true,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs(),
        updated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs(),
    };

    let mut fetch_root_user = conn.prepare("select id from users where root = true")?;

    match fetch_root_user.query_one([], |_row| Ok(())) {
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            conn.execute("insert into users (id, user_id, username, active_profile, account_type, root) values
                (?1, ?2, ?3,?4, ?5, ?6)", (&user.id.to_string(), &user.user_id, &user.username, &user.active_profile,
                    user.account_type.to_string(),  &user.root))?;
            Ok(())
        }
        Err(_err) => Err(anyhow!("Fetching user from db failed")),
        _ => Ok(()),
    }
}

fn create_root_user(root_user_config: &Table, nickname: Option<String>) -> Result<Table> {
    let mut root_user_table = root_user_config.clone();
    match create_identity("tiles") {
        Ok(did) => {
            root_user_table.insert("id".to_owned(), toml::Value::String(did));
            if let Some(nickname) = nickname {
                root_user_table.insert("nickname".to_owned(), toml::Value::String(nickname));
            }
            Ok(root_user_table)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::accounts::{
        RootUser, create_root_account, get_current_user, get_root_user_details, set_nickname,
    };
    use anyhow::Result;
    use keyring::{mock, set_default_credential_builder};
    use rusqlite::Connection;
    use toml::Table;

    #[test]
    fn test_get_root_user_details_empty_id() -> Result<()> {
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = ''
                nickname = ''
            "#,
        )
        .unwrap();
        let acc_details = get_root_user_details(&config)?;
        assert!(acc_details.id.is_empty());
        Ok(())
    }

    #[test]
    fn test_get_root_user_details_valid_id() -> Result<()> {
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = 'did:key:xyz'
                nickname = ''
            "#,
        )
        .unwrap();
        let acc_details = get_root_user_details(&config)?;
        assert!(acc_details.id.contains("did:key"));
        Ok(())
    }

    #[test]
    fn test_create_root_account_but_exists() {
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = 'did:key:xyz'
                nickname = ''
            "#,
        )
        .unwrap();
        let root_user = create_root_account(&config, None).unwrap();

        assert_eq!(
            root_user.get("id").unwrap().as_str().unwrap(),
            "did:key:xyz"
        );
    }

    #[test]
    fn test_create_root_account_new() {
        set_default_credential_builder(mock::default_credential_builder());
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = ''
                nickname = ''
            "#,
        )
        .unwrap();
        let root_user = create_root_account(&config, None).unwrap();

        assert_ne!(
            root_user.get("id").unwrap().as_str().unwrap(),
            "did:key:xyz"
        );

        assert!(
            root_user
                .get("id")
                .unwrap()
                .as_str()
                .unwrap()
                .starts_with("did:key")
        );
    }

    #[test]
    fn test_create_root_account_new_w_nickname() {
        set_default_credential_builder(mock::default_credential_builder());
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = ''
                nickname = ''
            "#,
        )
        .unwrap();
        let root_user = create_root_account(&config, Some(String::from("madclaws"))).unwrap();

        assert_ne!(
            root_user.get("id").unwrap().as_str().unwrap(),
            "did:key:xyz"
        );

        assert!(
            root_user
                .get("id")
                .unwrap()
                .as_str()
                .unwrap()
                .starts_with("did:key")
        );

        assert_eq!(
            root_user.get("nickname").unwrap().as_str().unwrap(),
            "madclaws"
        );
    }

    #[test]
    fn test_get_root_user_details_missing_key() {
        let config: Table = toml::from_str(
            r#"
                # no root-user table
                [other]
                foo = "bar"
            "#,
        )
        .unwrap();

        let res = get_root_user_details(&config);
        assert!(res.is_err(), "Expected error when root-user key is missing");
    }

    #[test]
    fn test_root_user_new_wrong_types() {
        // id is integer, nickname is table
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = 123
                nickname = { nested = "value" }
            "#,
        )
        .unwrap();

        let root_tbl = config.get("root-user").unwrap().as_table().unwrap().clone();
        assert!(
            RootUser::new(&root_tbl).is_err(),
            "Expected error for wrong types"
        );
    }

    #[test]
    fn test_root_user_roundtrip_table() -> Result<()> {
        let user = RootUser {
            id: "did:key:abc".into(),
            nickname: "nick".into(),
        };
        let tbl = user.to_table();
        let parsed = RootUser::new(&tbl)?;
        assert_eq!(parsed.id, user.id);
        assert_eq!(parsed.nickname, user.nickname);
        Ok(())
    }

    #[test]
    fn test_set_nickname_but_invalid_config() {
        let config: Table = toml::from_str(
            r#"
                [ruser]
                id = ''
            "#,
        )
        .unwrap();

        assert!(set_nickname(&config, "madclaws").is_err())
    }

    #[test]
    fn test_set_nickname_success() {
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = 'did:key:xyz'
                nickname = ''
            "#,
        )
        .unwrap();

        let updated = set_nickname(&config, "madclaws").expect("nickname update should succeed");
        assert_eq!(
            updated.get("id").and_then(|v| v.as_str()),
            Some("did:key:xyz")
        );
        assert_eq!(
            updated.get("nickname").and_then(|v| v.as_str()),
            Some("madclaws")
        );
    }

    #[test]
    fn test_set_nickname_with_empty_id_fails() {
        let config: Table = toml::from_str(
            r#"
                [root-user]
                id = ''
                nickname = ''
            "#,
        )
        .unwrap();

        let err = set_nickname(&config, "madclaws").expect_err("empty id should fail");
        assert!(err.to_string().contains("No Root user available"));
    }

    fn setup_db_schema() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
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
            [],
        )
        .unwrap();

        conn
    }

    #[test]
    fn test_get_user_when_no_user() {
        let conn = setup_db_schema();
        assert!(get_current_user(&conn).is_err())
    }

    #[test]
    fn test_get_current_user_valid() {
        let conn = setup_db_schema();
        let user = User {
            id: Uuid::now_v7(),
            user_id: String::from("did"),
            username: String::from("nickname"),
            account_type: ACCOUNT::LOCAL,
            active_profile: true,
            root: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs(),
            updated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs(),
        };

        let mut fetch_root_user = conn
            .prepare("select id from users where root = true")
            .unwrap();

        match fetch_root_user.query_one([], |_row| Ok(())) {
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                conn.execute("insert into users (id, user_id, username, active_profile, account_type, root) values
                (?1, ?2, ?3,?4, ?5, ?6)", (&user.id.to_string(), &user.user_id, &user.username, &user.active_profile,
                    user.account_type.to_string(),  &user.root)).unwrap();
            }
            Err(_err) => (),
            _ => (),
        }

        assert!(get_current_user(&conn).is_ok())
    }

    #[test]
    fn test_get_current_user_invalid_uuid_fails() {
        let conn = setup_db_schema();
        conn.execute(
            "insert into users (id, user_id, username, active_profile, account_type, root, created_at, updated_at)
            values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                "not-a-uuid",
                "did:key:test",
                "nickname",
                true,
                "local",
                true,
                1_i64,
                1_i64,
            ),
        )
        .unwrap();

        assert!(get_current_user(&conn).is_err());
    }

    #[test]
    fn test_get_current_user_invalid_account_type_fails() {
        let conn = setup_db_schema();
        conn.execute(
            "insert into users (id, user_id, username, active_profile, account_type, root, created_at, updated_at)
            values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                Uuid::now_v7().to_string(),
                "did:key:test",
                "nickname",
                true,
                "unknown",
                true,
                1_i64,
                1_i64,
            ),
        )
        .unwrap();

        assert!(get_current_user(&conn).is_err());
    }

    #[test]
    fn test_get_current_user_inactive_only_rows_fails() {
        let conn = setup_db_schema();
        conn.execute(
            "insert into users (id, user_id, username, active_profile, account_type, root, created_at, updated_at)
            values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                Uuid::now_v7().to_string(),
                "did:key:test",
                "nickname",
                false,
                "local",
                true,
                1_i64,
                1_i64,
            ),
        )
        .unwrap();

        assert!(get_current_user(&conn).is_err());
    }
}
