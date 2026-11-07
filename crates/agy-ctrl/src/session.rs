//! Project-specific conversation session reader and discovery engine for Antigravity.
//!
//! # Architecture
//! This module queries session metadata directly from `~/.gemini/antigravity-cli/conversation_summaries.db`
//! using embedded `rusqlite` with prepared parameterized statements.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Compact summary of a past conversation session for a specific agent in this workspace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct SessionSummary {
    pub(crate) conversation_id: String,
    #[serde(default)]
    pub(crate) agent_name: String,
    #[serde(default)]
    pub(crate) workspace_uris: String,
    #[serde(default)]
    pub(crate) last_modified_time: String,
    #[serde(default)]
    pub(crate) step_count: u64,
    #[serde(default)]
    pub(crate) preview: String,
}

impl SessionSummary {
    /// Formats a short prefix of the UUID conversation ID.
    #[must_use]
    pub(crate) fn short_id(&self) -> &str {
        self.conversation_id
            .get(..8)
            .unwrap_or(&self.conversation_id)
    }
}

/// Discovers the latest resumable session for a given agent in the given workspace.
///
/// Returns `None` if the session database does not exist or cannot be opened.
#[must_use]
pub(crate) fn find_agent_session_for_workspace(
    agent_name: &str,
    workspace_root: &Path,
) -> Option<SessionSummary> {
    let home = std::env::var("HOME").ok()?;
    let home_path = PathBuf::from(&home);
    let db_path = home_path.join(".gemini/antigravity-cli/conversation_summaries.db");
    let convs_dir = home_path.join(".gemini/antigravity-cli/conversations");

    query_session_from_db(&db_path, &convs_dir, agent_name, workspace_root)
}

fn query_session_from_db(
    db_path: &Path,
    convs_dir: &Path,
    agent_name: &str,
    workspace_root: &Path,
) -> Option<SessionSummary> {
    if !db_path.is_file() {
        return None;
    }

    let ws_canonical = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let ws_lossy = ws_canonical.to_string_lossy();
    let ws_pattern = format!("%{ws_lossy}%");

    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;

    let mut stmt = conn
        .prepare(
            "SELECT conversation_id, agent_name, workspace_uris, last_modified_time, step_count, preview \
             FROM conversation_summaries \
             WHERE agent_name = ?1 AND workspace_uris LIKE ?2 AND step_count > 0 \
             ORDER BY last_modified_time DESC LIMIT 5;",
        )
        .ok()?;

    let rows = stmt
        .query_map(rusqlite::params![agent_name, ws_pattern], |row| {
            Ok(SessionSummary {
                conversation_id: row.get(0)?,
                agent_name: row.get(1)?,
                workspace_uris: row.get(2)?,
                last_modified_time: row.get(3)?,
                step_count: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                preview: row.get(5)?,
            })
        })
        .ok()?;

    for row in rows.flatten() {
        let conv_file = convs_dir.join(format!("{}.db", row.conversation_id));
        if conv_file.is_file() {
            return Some(row);
        }
    }

    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_short_id() {
        let summary = SessionSummary {
            conversation_id: "6030b881-9107-4730-a943-fe383688402a".to_owned(),
            agent_name: "mock_agent".to_owned(),
            workspace_uris: "[]".to_owned(),
            last_modified_time: "2026-11-02".to_owned(),
            step_count: 10,
            preview: "Implement feature".to_owned(),
        };
        assert_eq!(summary.short_id(), "6030b881");
    }

    #[test]
    fn test_rusqlite_session_discovery() {
        let temp_dir = std::env::temp_dir().join(format!(
            "agy-ctrl-test-session-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let gemini_dir = temp_dir.join(".gemini/antigravity-cli");
        let convs_dir = gemini_dir.join("conversations");
        std::fs::create_dir_all(&convs_dir).unwrap();

        let db_path = gemini_dir.join("conversation_summaries.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE conversation_summaries (
                conversation_id TEXT PRIMARY KEY,
                agent_name TEXT,
                workspace_uris TEXT,
                last_modified_time TEXT,
                step_count INTEGER,
                preview TEXT
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO conversation_summaries VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "test-uuid-1234",
                "mock_agent",
                "[\"/home/infinite/projects/rust/agy-suite\"]",
                "2026-11-07T00:00:00Z",
                5,
                "Testing session"
            ],
        )
        .unwrap();

        std::fs::write(convs_dir.join("test-uuid-1234.db"), b"").unwrap();

        let ws = Path::new("/home/infinite/projects/rust/agy-suite");
        let res = query_session_from_db(&db_path, &convs_dir, "mock_agent", ws);
        assert!(res.is_some());
        let session = res.unwrap();
        assert_eq!(session.conversation_id, "test-uuid-1234");
        assert_eq!(session.step_count, 5);

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
