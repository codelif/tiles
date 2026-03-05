//! Chats.rs
//!
//! Stuff related to chats with the models
//!

use crate::core::accounts::User;
use crate::runtime::mlx::ChatResponse;
use crate::utils::get_unix_time_now;
use anyhow::Result;
use rusqlite::Connection;
use tilekit::modelfile::Role;
use uuid::Uuid;
// model the chats table
pub struct Chats {
    pub id: Uuid,
    content: String,
    // The id of the responses api obj
    response_id: Option<String>,
    // The Model chat user role
    role: Role,
    user_id: String,
    // The parent Id of a model's reply
    context_id: Option<Uuid>,
    created_at: u64,
    updated_at: u64,
}

pub fn save_chat(
    conn: &Connection,
    user: &User,
    input: &str,
    chat_resp: Option<&ChatResponse>,
) -> Result<Chats> {
    if let Some(chat_response) = chat_resp {
        let chat_resp_cloned = chat_response.clone();
        let chat = Chats {
            id: Uuid::now_v7(),
            user_id: user.user_id.clone(),
            content: input.to_owned(),
            response_id: Some(chat_resp_cloned.prev_response_id),
            role: Role::Assistant,
            context_id: chat_resp_cloned.parent_chat_id,
            created_at: get_unix_time_now(),
            updated_at: get_unix_time_now(),
        };

        conn.execute("insert into chats(id, user_id, content, resp_id, role, context_id, created_at, updated_at) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", (&chat.id.to_string(), &chat.user_id, &chat.content, &chat.response_id, Into::<String>::into(chat.role),  &chat.context_id.unwrap_or(Uuid::nil()).to_string(), &chat.created_at.to_string(), &chat.updated_at.to_string()))?;

        Ok(chat)
    } else {
        let chat = Chats {
            id: Uuid::now_v7(),
            user_id: user.user_id.clone(),
            content: input.to_owned(),
            response_id: None,
            role: Role::User,
            context_id: None,
            created_at: get_unix_time_now(),
            updated_at: get_unix_time_now(),
        };

        conn.execute("insert into chats(id, user_id, content, resp_id, role, context_id, created_at, updated_at) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", (&chat.id.to_string(), &chat.user_id, &chat.content, &chat.response_id, Into::<String>::into(chat.role),  &chat.context_id.unwrap_or(Uuid::nil()).to_string(), &chat.created_at.to_string(), &chat.updated_at.to_string()))?;

        Ok(chat)
    }
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
            chats::save_chat,
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
        assert_eq!(saved.context_id, Uuid::nil().to_string());
    }

    #[test]
    fn test_valid_response_save_chat() {
        let conn = setup_db_schema();
        let user = create_user();
        let parent_chat_id = Uuid::now_v7();
        let chat_resp = ChatResponse {
            reply: "reply".to_owned(),
            code: "code".to_owned(),
            prev_response_id: String::from("resp_prev"),
            parent_chat_id: Some(parent_chat_id),
            metrics: None,
        };
        let input = "2+2";
        let chat = save_chat(&conn, &user, input, Some(&chat_resp)).expect("chat should be saved");

        assert_eq!(chat.user_id, user.user_id);
        assert_eq!(chat.response_id.as_deref(), Some("resp_prev"));
        assert_eq!(chat.context_id, Some(parent_chat_id));

        let saved = fetch_saved_chat_row(&conn, &chat.id);
        assert_eq!(saved.content, input);
        assert_eq!(saved.resp_id, Some(String::from("resp_prev")));
        assert_eq!(saved.role, Into::<String>::into(Role::Assistant));
        assert_eq!(saved.user_id, user.user_id);
        assert_eq!(saved.context_id, parent_chat_id.to_string());
    }

    #[test]
    fn test_response_without_parent_chat_id_saves_nil_context() {
        let conn = setup_db_schema();
        let user = create_user();
        let chat_resp = ChatResponse {
            reply: "reply".to_owned(),
            code: "code".to_owned(),
            prev_response_id: String::from("resp_prev"),
            parent_chat_id: None,
            metrics: None,
        };

        let chat =
            save_chat(&conn, &user, "hello", Some(&chat_resp)).expect("chat should be saved");

        assert_eq!(chat.context_id, None);
        let saved = fetch_saved_chat_row(&conn, &chat.id);
        assert_eq!(saved.role, Into::<String>::into(Role::Assistant));
        assert_eq!(saved.context_id, Uuid::nil().to_string());
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

    struct SavedChatRow {
        content: String,
        resp_id: Option<String>,
        role: String,
        user_id: String,
        context_id: String,
    }

    fn fetch_saved_chat_row(conn: &Connection, chat_id: &Uuid) -> SavedChatRow {
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
        updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
    );",
            [],
        )
        .unwrap();

        conn
    }
}
