use std::path::Path;

use anyhow::Context;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};

use crate::agent::message::{Message, Role};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    db: SqlitePool,
}

impl SessionStore {
    pub async fn open(path: &str) -> anyhow::Result<Self> {
        if let Some(parent) = Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent).await?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let db = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .context("failed to open sqlite session store")?;

        let store = Self { db };
        store.migrate().await?;
        Ok(store)
    }

    pub async fn save_session(&self, id: &str, messages: &[Message]) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.begin().await?;

        sqlx::query(
            "INSERT INTO sessions (id, created_at, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        for (index, message) in messages.iter().enumerate() {
            sqlx::query(
                "INSERT INTO messages
                 (session_id, ordinal, role, content, tool_calls, tool_call_id)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(index as i64)
            .bind(message.role.as_str())
            .bind(&message.content)
            .bind(serde_json::to_string(&message.tool_calls)?)
            .bind(&message.tool_call_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn load_session(&self, id: &str) -> anyhow::Result<Vec<Message>> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
            "SELECT role, content, tool_calls, tool_call_id
             FROM messages
             WHERE session_id = ?
             ORDER BY ordinal ASC",
        )
        .bind(id)
        .fetch_all(&self.db)
        .await?;

        rows.into_iter()
            .map(|(role, content, tool_calls, tool_call_id)| {
                let role = match role.as_str() {
                    "system" => Role::System,
                    "user" => Role::User,
                    "assistant" => Role::Assistant,
                    "tool" => Role::Tool,
                    other => anyhow::bail!("unknown role {other}"),
                };
                Ok(Message {
                    role,
                    content,
                    tool_calls: tool_calls
                        .and_then(|value| serde_json::from_str(&value).ok())
                        .flatten(),
                    tool_call_id,
                })
            })
            .collect()
    }

    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        let rows = sqlx::query_as::<_, (String, String, String, i64)>(
            "SELECT s.id, s.created_at, s.updated_at, COUNT(m.id) AS message_count
             FROM sessions s
             LEFT JOIN messages m ON m.session_id = s.id
             GROUP BY s.id, s.created_at, s.updated_at
             ORDER BY s.updated_at DESC",
        )
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, created_at, updated_at, message_count)| SessionInfo {
                id,
                created_at,
                updated_at,
                message_count,
            })
            .collect())
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                tool_call_id TEXT,
                FOREIGN KEY(session_id) REFERENCES sessions(id)
            )",
        )
        .execute(&self.db)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_messages_session_ordinal
             ON messages(session_id, ordinal)",
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::message::{Message, Role};

    #[tokio::test]
    async fn save_load_and_list_roundtrip() {
        let path =
            std::env::temp_dir().join(format!("pi-hybrid-session-{}.db", uuid::Uuid::new_v4()));
        let store = SessionStore::open(path.to_str().unwrap()).await.unwrap();
        let messages = vec![
            Message::new(Role::User, "hello"),
            Message::new(Role::Assistant, "hi"),
        ];

        store.save_session("session-1", &messages).await.unwrap();
        let loaded = store.load_session("session-1").await.unwrap();
        let sessions = store.list_sessions().await.unwrap();

        assert_eq!(loaded, messages);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].message_count, 2);

        let _ = tokio::fs::remove_file(path).await;
    }
}
