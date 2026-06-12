//! Session Persistence — SQLite via sqlx.
//!
//! Stores conversation history, tool outputs, plan state, subagent results.
//! Import support for existing Pi JSON session files.
//!
//! Schema:
//! - sessions(id, created_at, model, status)
//! - messages(id, session_id, role, content, tool_calls, timestamp)
//! - plans(id, session_id, steps_json, status)

use anyhow::Context;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use std::path::Path;
use tracing::{debug, error, info, instrument};

/// A stored session record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub created_at: String,
    pub model: String,
    pub status: String,
}

/// A stored message record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<String>,
    pub timestamp: String,
}

/// A stored plan record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPlan {
    pub id: i64,
    pub session_id: i64,
    pub steps_json: String,
    pub status: String,
}

/// The SQLite-backed session store.
#[derive(Debug)]
pub struct SessionStore {
    pool: SqlitePool,
    /// The current active session id.
    current_session_id: Option<i64>,
}

impl SessionStore {
    /// Open or create the SQLite database and run migrations.
    #[instrument(skip(db_path))]
    pub async fn open(db_path: &str) -> anyhow::Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(db_path).parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent).await?;
        }

        info!(db_path, "Opening session store");

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .context("Failed to connect to SQLite database")?;

        let mut store = Self {
            pool,
            current_session_id: None,
        };

        store.run_migrations().await?;

        info!("Session store opened successfully");

        Ok(store)
    }

    /// Run schema migrations.
    async fn run_migrations(&self) -> anyhow::Result<()> {
        debug!("Running schema migrations");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL,
                model TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active'
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS plans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                steps_json TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'draft',
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
        )
        .execute(&self.pool)
        .await?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_plans_session_id ON plans(session_id)")
            .execute(&self.pool)
            .await?;

        debug!("Schema migrations complete");

        Ok(())
    }

    /// Create a new session and set it as current.
    pub async fn create_session(&mut self, model: &str) -> anyhow::Result<i64> {
        let now = Utc::now().to_rfc3339();
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO sessions (created_at, model, status) VALUES (?, ?, 'active') RETURNING id",
        )
        .bind(&now)
        .bind(model)
        .fetch_one(&self.pool)
        .await?;

        self.current_session_id = Some(id);
        debug!(session_id = id, model, "Session created");
        Ok(id)
    }

    /// Set the current session by id.
    pub async fn set_current_session(&mut self, session_id: i64) -> anyhow::Result<()> {
        // Verify the session exists
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions WHERE id = ?")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;

        if exists > 0 {
            self.current_session_id = Some(session_id);
            Ok(())
        } else {
            anyhow::bail!("Session {session_id} not found");
        }
    }

    /// Add a message to the current session.
    pub async fn add_message(
        &self,
        role: &str,
        content: &str,
        tool_calls: Option<&str>,
    ) -> anyhow::Result<i64> {
        let session_id = self
            .current_session_id
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;
        let now = Utc::now().to_rfc3339();

        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO messages (session_id, role, content, tool_calls, timestamp) VALUES (?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(tool_calls)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Get messages for a session, ordered by timestamp.
    pub async fn get_messages(
        &self,
        session_id: i64,
        limit: Option<i64>,
    ) -> anyhow::Result<Vec<StoredMessage>> {
        let limit = limit.unwrap_or(1000);
        let rows = sqlx::query_as::<_, (i64, i64, String, String, Option<String>, String)>(
            "SELECT id, session_id, role, content, tool_calls, timestamp FROM messages WHERE session_id = ? ORDER BY timestamp ASC LIMIT ?",
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let messages = rows
            .into_iter()
            .map(
                |(id, session_id, role, content, tool_calls, timestamp)| StoredMessage {
                    id,
                    session_id,
                    role,
                    content,
                    tool_calls,
                    timestamp,
                },
            )
            .collect();

        Ok(messages)
    }

    /// Save a plan to the current session.
    pub async fn save_plan(&self, steps_json: &str, status: &str) -> anyhow::Result<i64> {
        let session_id = self
            .current_session_id
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;

        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO plans (session_id, steps_json, status) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(session_id)
        .bind(steps_json)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Get the most recent plan for a session.
    pub async fn get_latest_plan(&self, session_id: i64) -> anyhow::Result<Option<StoredPlan>> {
        let row = sqlx::query_as::<_, (i64, i64, String, String)>(
            "SELECT id, session_id, steps_json, status FROM plans WHERE session_id = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(id, session_id, steps_json, status)| StoredPlan {
            id,
            session_id,
            steps_json,
            status,
        }))
    }

    /// List all sessions, most recent first.
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<Session>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String)>(
            "SELECT id, created_at, model, status FROM sessions ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, created_at, model, status)| Session {
                id,
                created_at,
                model,
                status,
            })
            .collect())
    }

    /// Update session status.
    pub async fn update_session_status(&self, session_id: i64, status: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE sessions SET status = ? WHERE id = ?")
            .bind(status)
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get the number of messages in the current session.
    pub async fn message_count(&self) -> anyhow::Result<i64> {
        let session_id = self
            .current_session_id
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;
        let count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM messages WHERE session_id = ?")
                .bind(session_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }

    /// Import a session from a Pi JSON session file.
    /// Expected format: {"messages": [...], "model": "...", ...}
    pub async fn import_json_session(&mut self, json_path: &str) -> anyhow::Result<i64> {
        let content = tokio::fs::read_to_string(json_path)
            .await
            .context("Failed to read JSON session file")?;

        let parsed: serde_json::Value =
            serde_json::from_str(&content).context("Failed to parse JSON session file")?;

        let model = parsed
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let session_id = self.create_session(model).await?;

        if let Some(messages) = parsed.get("messages").and_then(|v| v.as_array()) {
            for msg in messages {
                let role = msg
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let tool_calls = msg
                    .get("tool_calls")
                    .and_then(|v| serde_json::to_string(v).ok());

                self.add_message(role, content, tool_calls.as_deref())
                    .await?;
            }
        }

        // Update status to 'imported'
        self.update_session_status(session_id, "imported").await?;

        Ok(session_id)
    }

    /// Export the current session to a JSON file.
    pub async fn export_json(&self, output_path: &str) -> anyhow::Result<()> {
        let session_id = self
            .current_session_id
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;

        let messages = self.get_messages(session_id, None).await?;
        let sessions = sqlx::query_as::<_, (String, String)>(
            "SELECT model, status FROM sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;

        let export = serde_json::json!({
            "session_id": session_id,
            "model": sessions.0,
            "status": sessions.1,
            "messages": messages.iter().map(|m| {
                let mut map = serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                    "timestamp": m.timestamp,
                });
                if let Some(ref tc) = m.tool_calls
                    && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(tc) {
                        map["tool_calls"] = parsed;
                    }
                map
            }).collect::<Vec<_>>(),
        });

        let json = serde_json::to_string_pretty(&export)?;
        tokio::fs::write(output_path, json).await?;

        Ok(())
    }

    /// Get the current session id.
    pub fn current_session_id(&self) -> Option<i64> {
        self.current_session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_store() -> SessionStore {
        // Use a unique in-memory database per test
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        let mut store = SessionStore {
            pool,
            current_session_id: None,
        };
        store.run_migrations().await.unwrap();
        store
    }

    #[tokio::test]
    async fn create_and_list_sessions() {
        let mut store = create_test_store().await;

        let id = store.create_session("test-model").await.unwrap();
        assert_eq!(store.current_session_id(), Some(id));

        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].model, "test-model");
    }

    #[tokio::test]
    async fn add_and_retrieve_messages() {
        let mut store = create_test_store().await;
        store.create_session("test-model").await.unwrap();

        store.add_message("user", "Hello", None).await.unwrap();
        store
            .add_message("assistant", "Hi there!", None)
            .await
            .unwrap();

        let session_id = store.current_session_id().unwrap();
        let messages = store.get_messages(session_id, None).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
    }

    #[tokio::test]
    async fn save_and_retrieve_plan() {
        let mut store = create_test_store().await;
        store.create_session("test-model").await.unwrap();

        let steps_json = r#"[{"description":"Do thing","tool":"shell","params":{}}]"#;
        store.save_plan(steps_json, "draft").await.unwrap();

        let session_id = store.current_session_id().unwrap();
        let plan = store.get_latest_plan(session_id).await.unwrap();
        assert!(plan.is_some());
        assert_eq!(plan.unwrap().steps_json, steps_json);
    }

    #[tokio::test]
    async fn message_count_works() {
        let mut store = create_test_store().await;
        store.create_session("test-model").await.unwrap();

        assert_eq!(store.message_count().await.unwrap(), 0);
        store.add_message("user", "msg1", None).await.unwrap();
        store.add_message("user", "msg2", None).await.unwrap();
        assert_eq!(store.message_count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn import_json_session() {
        let mut store = create_test_store().await;

        let json_content = r#"{
            "model": "claude-3",
            "messages": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi!"}
            ]
        }"#;

        // Write to temp file
        let tmp = std::env::temp_dir().join("test_pi_session.json");
        tokio::fs::write(&tmp, json_content).await.unwrap();

        let id = store
            .import_json_session(tmp.to_str().unwrap())
            .await
            .unwrap();
        let messages = store.get_messages(id, None).await.unwrap();
        assert_eq!(messages.len(), 2);

        // Clean up
        let _ = tokio::fs::remove_file(&tmp).await;
    }

    #[tokio::test]
    async fn update_session_status() {
        let mut store = create_test_store().await;
        let id = store.create_session("test-model").await.unwrap();

        store.update_session_status(id, "completed").await.unwrap();

        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions[0].status, "completed");
    }

    #[tokio::test]
    async fn no_active_session_errors() {
        let store = create_test_store().await;
        let result = store.add_message("user", "test", None).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No active session")
        );
    }
}
