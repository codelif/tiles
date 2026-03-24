//! Chats.rs
//!
//! Stuff related to chats with the models
//!

use std::str::FromStr;

use crate::core::accounts::User;
use crate::runtime::mlx::ChatResponse;
use crate::utils::get_unix_time_now;
use anyhow::Result;
use rusqlite::types::FromSqlError;
use rusqlite::{Connection, params};
use tilekit::modelfile::Role;
use uuid::Uuid;
// model the chats table

// TODO: foreign types on foreign traits, lul
// someday we can do this for traits sake
// https://dev.to/iprosk/generics-in-rust-murky-waters-of-implementing-foreign-traits-on-foreign-types-584n

// impl FromSql for Uuid {
//     fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
//         let value_str = String::column_result(value)?;
//         Uuid::from_str(&value_str).map_err(|_| FromSqlError::InvalidType)
//     }
// }

#[derive(serde::Serialize, Clone, Debug)]
pub struct Message {
    pub r#type: String,
    pub role: Role,
    pub content: String,
}

#[derive(Debug)]
pub struct Chats {
    pub id: String,
    content: String,
    // The id of the responses api obj
    response_id: Option<String>,
    // The Model chat user role
    role: Role,
    user_id: String,
    // The parent Id of a model's reply
    context_id: Option<String>,
    created_at: u64,
    updated_at: u64,
    row_counter: i64,
}

pub fn save_chat(
    conn: &Connection,
    user: &User,
    input: &str,
    chat_resp: Option<&ChatResponse>,
) -> Result<Chats> {
    let row_counter = get_last_row_counter(conn, &user.user_id)?;
    if let Some(chat_response) = chat_resp {
        let chat_resp_cloned = chat_response.clone();

        let chat = Chats {
            id: Uuid::now_v7().to_string(),
            user_id: user.user_id.clone(),
            content: input.to_owned(),
            response_id: Some(chat_resp_cloned.prev_response_id),
            role: Role::Assistant,
            context_id: chat_resp_cloned.parent_chat_id,
            created_at: get_unix_time_now(),
            updated_at: get_unix_time_now(),
            row_counter: row_counter + 1,
        };

        conn.execute("insert into chats(id, user_id, content, resp_id, role, context_id, created_at, updated_at, row_counter) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", (&chat.id.to_string(), &chat.user_id, &chat.content, &chat.response_id, Into::<String>::into(chat.role),  &chat.context_id, &chat.created_at.to_string(), &chat.updated_at.to_string(), &chat.row_counter))?;

        Ok(chat)
    } else {
        let chat = Chats {
            id: Uuid::now_v7().to_string(),
            user_id: user.user_id.clone(),
            content: input.to_owned(),
            response_id: None,
            role: Role::User,
            context_id: None,
            created_at: get_unix_time_now(),
            updated_at: get_unix_time_now(),
            row_counter: row_counter + 1,
        };

        conn.execute("insert into chats(id, user_id, content, resp_id, role, context_id, created_at, updated_at, row_counter) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", (&chat.id,  &chat.user_id, &chat.content, &chat.response_id, Into::<String>::into(chat.role), &chat.context_id, &chat.created_at.to_string(), &chat.updated_at.to_string(), &chat.row_counter))?;

        Ok(chat)
    }
}

/// Returns the `id` of the last entry of the given user_id
/// Used as the offset point for fetching the chat delta from the user_id
pub fn get_last_entry_id(conn: &Connection, user_id: &str) -> Result<Option<Uuid>> {
    match conn.query_row(
        "select id from chats where user_id = ?1 order by id desc limit 1",
        [user_id],
        |row| row.get::<usize, String>(0),
    ) {
        Ok(res) => Uuid::from_str(&res).map_err(Into::into).map(Some),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(<rusqlite::Error as Into<anyhow::Error>>::into(err)),
    }
}

/// Returns the `row_counter` of the last entry of the given user_id
/// Used as the offset point for fetching the chat delta from the user_id
pub fn get_last_row_counter(conn: &Connection, user_id: &str) -> Result<i64> {
    match conn.query_row(
        "select max(row_counter) from chats where user_id = ?1",
        [user_id],
        |row| row.get::<usize, i64>(0),
    ) {
        Ok(res) => Ok(res),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        // It returns NULL, if there are now no rows
        Err(rusqlite::Error::InvalidColumnType(_, _, _)) => Ok(0),
        Err(err) => Err(<rusqlite::Error as Into<anyhow::Error>>::into(err)),
    }
}
/// Return list of rows..
/// encoding is the job of network modules
pub fn get_delta(conn: &Connection, user_id: &str, last_row_couter: i64) -> Result<Vec<Chats>> {
    let mut stmt = conn.prepare("select id, user_id, content, resp_id, role, context_id, created_at, updated_at , row_counter from chats where user_id = ?1 and row_counter > ?2 order by id")?;

    let chat_rows = stmt.query_map(params![user_id, last_row_couter], |row| {
        let id: String = row.get(0)?;
        let role: String = row.get(4)?;
        let created_at: f64 = row.get(6)?;
        let updated_at: f64 = row.get(7)?;
        let resp_id: Option<String> = row.get(3)?;
        let ctx_id = row.get(5)?;
        Ok(Chats {
            id,
            content: row.get(2)?,
            response_id: resp_id,
            role: Role::from_str(&role).map_err(FromSqlError::other)?,
            user_id: row.get(1)?,
            context_id: ctx_id,
            created_at: created_at as u64,
            updated_at: updated_at as u64,
            row_counter: row.get(8)?,
        })
    })?;

    let mut chats: Vec<Chats> = vec![];

    for chat in chat_rows {
        chats.push(chat?);
    }

    Ok(chats)
}

pub fn apply_delta(chat_conn: &mut Connection, delta_chats: Vec<Chats>) -> Result<()> {
    // bulk insert
    // TODO: Handle primary key conflict, for now upsert it, later
    // do LWW based on iss of UCAN
    let txn = chat_conn.transaction()?;
    {
        let mut stmt = txn.prepare("insert into chats(id, user_id, content, resp_id, role, context_id, created_at, updated_at, row_counter) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?;

        for chat in delta_chats {
            stmt.execute(params![
                &chat.id.to_string(),
                &chat.user_id,
                &chat.content,
                &chat.response_id,
                Into::<String>::into(chat.role),
                &chat.context_id,
                &chat.created_at.to_string(),
                &chat.updated_at.to_string(),
                &chat.row_counter,
            ])?;
        }
    }
    txn.commit()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;
    use tilekit::modelfile::Role;
    use uuid::Uuid;

    use crate::{
        core::{
            accounts::{ACCOUNT, User},
            chats::{apply_delta, get_delta, get_last_row_counter, save_chat},
        },
        runtime::mlx::ChatResponse,
    };

    #[test]
    fn test_valid_input_save_chat() {
        let conn = setup_db_schema();
        let user = create_user();
        let input = "2+2";
        let chat = save_chat(&conn, &user, input, None).expect("chat should be saved");

        assert_eq!(chat.user_id, user.user_id);
        assert!(chat.response_id.is_none());
        assert!(chat.context_id.is_none());

        let saved = fetch_saved_chat_row(&conn, &chat.id);
        assert_eq!(saved.content, input);
        assert_eq!(saved.resp_id, None);
        assert_eq!(saved.role, Into::<String>::into(Role::User));
        assert_eq!(saved.user_id, user.user_id);
        assert_eq!(saved.context_id, None);
    }

    #[test]
    fn test_valid_response_save_chat() {
        let conn = setup_db_schema();
        let user = create_user();
        let parent_chat_id = Uuid::now_v7().to_string();
        let chat_resp = ChatResponse {
            reply: "reply".to_owned(),
            code: "code".to_owned(),
            prev_response_id: String::from("resp_prev"),
            parent_chat_id: Some(parent_chat_id.clone()),
            metrics: None,
        };
        let input = "2+2";
        let chat = save_chat(&conn, &user, input, Some(&chat_resp)).expect("chat should be saved");

        assert_eq!(chat.user_id, user.user_id);
        assert_eq!(chat.response_id.as_deref(), Some("resp_prev"));
        assert_eq!(chat.context_id, Some(parent_chat_id.clone()));

        let saved = fetch_saved_chat_row(&conn, &chat.id);
        assert_eq!(saved.content, input);
        assert_eq!(saved.resp_id, Some(String::from("resp_prev")));
        assert_eq!(saved.role, Into::<String>::into(Role::Assistant));
        assert_eq!(saved.user_id, user.user_id);
        assert_eq!(saved.context_id, Some(parent_chat_id.clone()));
    }

    #[test]
    fn test_response_without_parent_chat_id_saves_nil_context() {
        let conn = setup_db_schema();
        let user = create_user();
        let chat_resp = ChatResponse {
            reply: "reply".to_owned(),
            code: "code".to_owned(),
            prev_response_id: String::from("resp_prev"),
            parent_chat_id: Some(Uuid::now_v7().to_string()),
            metrics: None,
        };

        let chat =
            save_chat(&conn, &user, "hello", Some(&chat_resp)).expect("chat should be saved");

        assert!(chat.context_id.is_some());
        let saved = fetch_saved_chat_row(&conn, &chat.id);
        assert_eq!(saved.role, Into::<String>::into(Role::Assistant));
        assert!(saved.context_id.is_some());
    }

    #[test]
    fn test_empty_input_is_saved() {
        let conn = setup_db_schema();
        let user = create_user();

        let chat = save_chat(&conn, &user, "", None).expect("empty content should still be saved");

        let saved = fetch_saved_chat_row(&conn, &chat.id);
        assert_eq!(saved.content, "");
        assert_eq!(saved.role, Into::<String>::into(Role::User));
    }

    #[test]
    fn test_save_chat_errors_when_table_missing() {
        let conn = Connection::open_in_memory().expect("in-memory db should open");
        let user = create_user();

        let result = save_chat(&conn, &user, "2+2", None);

        assert!(result.is_err());
    }

    #[test]
    fn test_last_row_counter() {
        let conn = setup_db_schema();
        let user = create_user();
        let input = "2+2";
        let chat = save_chat(&conn, &user, input, None).expect("chat should be saved");

        assert_eq!(chat.user_id, user.user_id);
        assert!(chat.response_id.is_none());
        assert!(chat.context_id.is_none());

        let saved = get_last_row_counter(&conn, &user.user_id);
        assert_eq!(saved.unwrap(), 1);
    }

    #[test]
    fn test_get_last_row_counter_without_entry() {
        let conn = setup_db_schema();
        let user = create_user();
        let saved = get_last_row_counter(&conn, &user.user_id);
        assert_eq!(saved.unwrap(), 0)
    }

    #[test]
    fn test_get_delta_diff() {
        let conn = setup_db_schema();
        let user = create_user();
        let input = "2+2";
        let chat_1 = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");

        let rows = get_delta(&conn, &user.user_id, chat_1.row_counter).unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn test_get_delta_diff_empty_last_entry_id() {
        let conn = setup_db_schema();
        let user = create_user();
        let input = "2+2";
        let _chat_1 = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");

        let rows = get_delta(&conn, &user.user_id, 0).unwrap();
        assert_eq!(rows.len(), 4);
    }

    #[test]
    fn test_get_delta_diff_empty_wrong_user_id() {
        let conn = setup_db_schema();
        let user = create_user();
        let input = "2+2";
        let _chat_1 = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");

        let rows = get_delta(&conn, "", 0).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn test_apply_delta() {
        let conn = setup_db_schema();
        let mut conn_2 = setup_db_schema();
        let user = create_user();
        let input = "2+2";
        let _chat_1 = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");
        let _ = save_chat(&conn, &user, input, None).expect("chat should be saved");

        let rows = get_delta(&conn, &user.user_id, 0).unwrap();
        assert_eq!(rows.len(), 4);
        assert!(apply_delta(&mut conn_2, rows).is_ok());
        let rows = get_delta(&conn_2, &user.user_id, 0).unwrap();
        assert_eq!(rows.len(), 4);
    }

    struct SavedChatRow {
        content: String,
        resp_id: Option<String>,
        role: String,
        user_id: String,
        context_id: Option<String>,
    }

    fn fetch_saved_chat_row(conn: &Connection, chat_id: &str) -> SavedChatRow {
        conn.query_row(
            "SELECT content, resp_id, role, user_id, context_id FROM chats WHERE id = ?1",
            [chat_id.to_string()],
            |row| {
                Ok(SavedChatRow {
                    content: row.get(0)?,
                    resp_id: row.get(1)?,
                    role: row.get(2)?,
                    user_id: row.get(3)?,
                    context_id: row.get(4)?,
                })
            },
        )
        .expect("saved chat row should exist")
    }

    fn create_user() -> User {
        User {
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
        }
    }
    fn setup_db_schema() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chats (
        id TEXT PRIMARY KEY,
        content TEXT NOT NULL,
        resp_id TEXT,
        role TEXT NOT NULL,
        user_id TEXT NOT NULL,
        context_id TEXT ,
        created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        row_counter INTEGER,
        session_id TEXT
    );",
            [],
        )
        .unwrap();

        conn
    }
}
