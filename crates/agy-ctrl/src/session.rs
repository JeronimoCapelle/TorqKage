//! Project-specific conversation session reader and discovery engine for Antigravity.
//!
//! # Architecture
//! This module discovers past conversation sessions for an agent within the active workspace.
//! Metadata is resolved across multiple Antigravity storage layers:
//! 1. Direct workspace URI matching in `~/.gemini/antigravity-cli/conversation_summaries.db`
//! 2. Prompt execution history in `~/.gemini/antigravity-cli/history.jsonl` (CLI sessions)
//! 3. Full-text search catalog in `conversations_fts` (FTS5 index)

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

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

#[derive(Deserialize)]
struct HistoryEntry {
    #[serde(rename = "conversationId")]
    conversation_id: Option<String>,
    workspace: Option<PathBuf>,
}

fn load_history_workspaces(history_path: &Path) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let Ok(file) = File::open(history_path) else {
        return map;
    };
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line)
            && let (Some(cid), Some(ws)) = (entry.conversation_id, entry.workspace)
        {
            map.insert(cid, ws);
        }
    }
    map
}

fn workspace_matches(ws_a: &Path, ws_b: &Path) -> bool {
    if ws_a == ws_b {
        return true;
    }
    let canon_a = ws_a.canonicalize().unwrap_or_else(|_| ws_a.to_path_buf());
    let canon_b = ws_b.canonicalize().unwrap_or_else(|_| ws_b.to_path_buf());
    canon_a == canon_b || canon_a.starts_with(&canon_b) || canon_b.starts_with(&canon_a)
}

fn session_matches_workspace(
    conn: &rusqlite::Connection,
    row: &SessionSummary,
    workspace_root: &Path,
    ws_canonical: &Path,
    history_map: &HashMap<String, PathBuf>,
) -> bool {
    // 1. Direct workspace_uris match (e.g. from IDE or multi-root workspace flag)
    if !row.workspace_uris.is_empty() {
        let ws_str = ws_canonical.to_string_lossy();
        if row.workspace_uris.contains(&*ws_str) {
            return true;
        }
        let root_str = workspace_root.to_string_lossy();
        if row.workspace_uris.contains(&*root_str) {
            return true;
        }
    }

    // 2. CLI interaction history match (history.jsonl records working directory for each turn)
    if let Some(hist_ws) = history_map.get(&row.conversation_id)
        && workspace_matches(hist_ws, ws_canonical)
    {
        return true;
    }

    // 3. SQLite FTS workspace match fallback
    if let Ok(mut fts_stmt) = conn.prepare_cached(
        "SELECT workspace FROM conversations_fts WHERE conversation_id = ?1 LIMIT 1;",
    ) {
        let fts_ws: Option<String> = fts_stmt
            .query_row(rusqlite::params![row.conversation_id], |r| r.get(0))
            .ok();
        if let Some(ws) = fts_ws
            && !ws.is_empty()
        {
            let ws_path = Path::new(&ws);
            if workspace_matches(ws_path, ws_canonical) {
                return true;
            }
            if let Some(ws_name) = ws_canonical.file_name().and_then(|n| n.to_str())
                && ws == ws_name
            {
                return true;
            }
        }
    }

    false
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
    let history_path = home_path.join(".gemini/antigravity-cli/history.jsonl");

    query_session_from_sources(
        &db_path,
        &convs_dir,
        &history_path,
        agent_name,
        workspace_root,
    )
}

pub(crate) fn query_session_from_sources(
    db_path: &Path,
    convs_dir: &Path,
    history_path: &Path,
    agent_name: &str,
    workspace_root: &Path,
) -> Option<SessionSummary> {
    if !db_path.is_file() {
        return None;
    }

    let ws_canonical = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());

    let history_map = load_history_workspaces(history_path);

    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;

    let mut stmt = conn
        .prepare(
            "SELECT conversation_id, agent_name, workspace_uris, last_modified_time, step_count, preview \
             FROM conversation_summaries \
             WHERE LOWER(agent_name) = LOWER(?1) AND step_count > 0 \
             ORDER BY last_modified_time DESC LIMIT 20;",
        )
        .ok()?;

    let rows = stmt
        .query_map(rusqlite::params![agent_name], |row| {
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
        if !conv_file.is_file() {
            continue;
        }

        if session_matches_workspace(&conn, &row, workspace_root, &ws_canonical, &history_map) {
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
    fn test_rusqlite_session_discovery_via_workspace_uris() {
        let temp_dir = std::env::temp_dir().join(format!(
            "agy-ctrl-test-session-uris-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let gemini_dir = temp_dir.join(".gemini/antigravity-cli");
        let convs_dir = gemini_dir.join("conversations");
        std::fs::create_dir_all(&convs_dir).unwrap();

        let db_path = gemini_dir.join("conversation_summaries.db");
        let history_path = gemini_dir.join("history.jsonl");
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
        let res = query_session_from_sources(&db_path, &convs_dir, &history_path, "mock_agent", ws);
        assert!(res.is_some());
        let session = res.unwrap();
        assert_eq!(session.conversation_id, "test-uuid-1234");
        assert_eq!(session.step_count, 5);

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_rusqlite_session_discovery_via_history_jsonl() {
        let temp_dir = std::env::temp_dir().join(format!(
            "agy-ctrl-test-session-hist-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let gemini_dir = temp_dir.join(".gemini/antigravity-cli");
        let convs_dir = gemini_dir.join("conversations");
        std::fs::create_dir_all(&convs_dir).unwrap();

        let db_path = gemini_dir.join("conversation_summaries.db");
        let history_path = gemini_dir.join("history.jsonl");
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

        // Empty workspace_uris, mimicking CLI interactive session behavior
        conn.execute(
            "INSERT INTO conversation_summaries VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "cli-uuid-5678",
                "copilot",
                "",
                "2026-11-07T23:00:00Z",
                47,
                "Setup Test And Capabilities"
            ],
        )
        .unwrap();

        // history.jsonl records the workspace path
        let hist_entry = serde_json::json!({
            "conversationId": "cli-uuid-5678",
            "workspace": "/home/infinite/projects/rust/Pure-V",
            "display": "Test prompt"
        });
        std::fs::write(&history_path, format!("{hist_entry}\n")).unwrap();

        std::fs::write(convs_dir.join("cli-uuid-5678.db"), b"").unwrap();

        let ws_match = Path::new("/home/infinite/projects/rust/Pure-V");
        let res =
            query_session_from_sources(&db_path, &convs_dir, &history_path, "copilot", ws_match);
        assert!(res.is_some());
        let session = res.unwrap();
        assert_eq!(session.conversation_id, "cli-uuid-5678");
        assert_eq!(session.step_count, 47);

        // Mismatched workspace should return None
        let ws_mismatch = Path::new("/home/infinite/projects/rust/other-repo");
        let res_none =
            query_session_from_sources(&db_path, &convs_dir, &history_path, "copilot", ws_mismatch);
        assert!(res_none.is_none());

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_live_session_discovery_pure_v() {
        let ws = Path::new("/home/infinite/projects/rust/Pure-V");
        if ws.is_dir() {
            let session = find_agent_session_for_workspace("copilot", ws);
            assert!(
                session.is_some(),
                "Expected copilot session f1bb6355 to be found for Pure-V workspace"
            );
            let s = session.unwrap();
            assert_eq!(s.conversation_id, "f1bb6355-7b30-4670-970e-2a56fb49cae1");
            assert!(s.step_count >= 47);
        }
    }
}
