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

/// Abstract session provider contract for discovering prior agent conversations.
pub(crate) trait SessionProvider: Send + Sync {
    /// Discovers the latest resumable session for a given agent in the given workspace.
    fn find_agent_session(&self, agent_name: &str, workspace_root: &Path)
    -> Option<SessionSummary>;
}

/// Native Antigravity SQLite session provider.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AntigravitySessionProvider;

impl SessionProvider for AntigravitySessionProvider {
    fn find_agent_session(
        &self,
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
}

/// `OpenCode` session provider discovering `.opencode/sessions` and user directory traces.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct OpenCodeSessionProvider;

impl SessionProvider for OpenCodeSessionProvider {
    fn find_agent_session(
        &self,
        agent_name: &str,
        workspace_root: &Path,
    ) -> Option<SessionSummary> {
        discover_opencode_session(agent_name, workspace_root)
    }
}

/// Generic state session provider reading `.tkg/state/sessions`.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GenericSessionProvider;

impl SessionProvider for GenericSessionProvider {
    fn find_agent_session(
        &self,
        agent_name: &str,
        workspace_root: &Path,
    ) -> Option<SessionSummary> {
        discover_generic_state_session(agent_name, workspace_root)
    }
}

/// Factory creating a `SessionProvider` dynamically matching the configured execution harness.
#[must_use]
pub(crate) fn create_session_provider(
    runner_cfg: &tkg_common::ProjectRunnerConfig,
) -> Box<dyn SessionProvider> {
    crate::installer::driver_for(runner_cfg).session_provider()
}

/// Discovers the latest resumable session for a given agent in the given workspace using explicit runner settings.
#[must_use]
pub(crate) fn find_agent_session_with_runner(
    agent_name: &str,
    workspace_root: &Path,
    runner_cfg: &tkg_common::ProjectRunnerConfig,
) -> Option<SessionSummary> {
    let provider = create_session_provider(runner_cfg);
    provider.find_agent_session(agent_name, workspace_root)
}

/// Discovers the latest resumable session for a given agent in the given workspace.
///
/// Automatically inspects `project.toml` to select the appropriate session backend.
#[must_use]
pub(crate) fn find_agent_session_for_workspace(
    agent_name: &str,
    workspace_root: &Path,
) -> Option<SessionSummary> {
    let project_cfg = tkg_common::ProjectConfig::detect_or_load(workspace_root);
    let runner = project_cfg.resolved_runner();
    find_agent_session_with_runner(agent_name, workspace_root, &runner)
}

fn discover_opencode_session(agent_name: &str, workspace_root: &Path) -> Option<SessionSummary> {
    let mut candidate_dirs = Vec::new();
    candidate_dirs.push(workspace_root.join(".opencode").join("sessions"));
    candidate_dirs.push(workspace_root.join(".opencode"));

    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        candidate_dirs.push(home_path.join(".local/share/opencode/sessions"));
        candidate_dirs.push(home_path.join(".opencode/sessions"));
    }

    let mut sessions = Vec::new();
    for dir in candidate_dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(content) = std::fs::read_to_string(&path)
                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(sess) = parse_json_session(&val, &path, agent_name)
            {
                sessions.push(sess);
            }
        }
    }

    sessions.sort_by(|a, b| b.last_modified_time.cmp(&a.last_modified_time));
    sessions.into_iter().next()
}

fn discover_generic_state_session(
    agent_name: &str,
    workspace_root: &Path,
) -> Option<SessionSummary> {
    let state_dir = workspace_root.join(".tkg/state/sessions");
    if !state_dir.is_dir() {
        return None;
    }
    let Ok(entries) = std::fs::read_dir(state_dir) else {
        return None;
    };
    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json")
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(sess) = parse_json_session(&val, &path, agent_name)
        {
            sessions.push(sess);
        }
    }

    sessions.sort_by(|a, b| b.last_modified_time.cmp(&a.last_modified_time));
    sessions.into_iter().next()
}

fn parse_json_session(
    val: &serde_json::Value,
    path: &Path,
    agent_name: &str,
) -> Option<SessionSummary> {
    let declared_agent = val
        .get("agent")
        .or_else(|| val.get("agent_name"))
        .or_else(|| val.get("agentName"))
        .and_then(serde_json::Value::as_str);

    if let Some(da) = declared_agent
        && !da.trim().is_empty()
        && !da.eq_ignore_ascii_case(agent_name)
    {
        return None;
    }

    let conversation_id = val
        .get("id")
        .or_else(|| val.get("session_id"))
        .or_else(|| val.get("sessionId"))
        .or_else(|| val.get("conversation_id"))
        .and_then(serde_json::Value::as_str)
        .map_or_else(
            || {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_owned()
            },
            ToOwned::to_owned,
        );

    let last_modified_time = val
        .get("updated_at")
        .or_else(|| val.get("updatedAt"))
        .or_else(|| val.get("last_modified_time"))
        .or_else(|| val.get("timestamp"))
        .and_then(serde_json::Value::as_str)
        .map_or_else(
            || {
                path.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map_or_else(
                        || "1970-01-01T00:00:00Z".to_owned(),
                        |t| {
                            let dur = t
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            dur.to_string()
                        },
                    )
            },
            ToOwned::to_owned,
        );

    let step_count = val
        .get("step_count")
        .or_else(|| val.get("stepCount"))
        .and_then(serde_json::Value::as_u64)
        .or_else(|| {
            let arr = val
                .get("messages")
                .or_else(|| val.get("turns"))
                .or_else(|| val.get("steps"))
                .and_then(serde_json::Value::as_array)?;
            u64::try_from(arr.len()).ok()
        })
        .unwrap_or(1);

    if step_count == 0 {
        return None;
    }

    let preview = val
        .get("preview")
        .or_else(|| val.get("title"))
        .or_else(|| val.get("summary"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_owned();

    Some(SessionSummary {
        conversation_id,
        agent_name: agent_name.to_owned(),
        workspace_uris: String::new(),
        last_modified_time,
        step_count,
        preview,
    })
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
            "tkg-ctrl-test-session-uris-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock_ws = temp_dir.join("workspace_a");
        let gemini_dir = temp_dir.join(".gemini/antigravity-cli");
        let convs_dir = gemini_dir.join("conversations");
        std::fs::create_dir_all(&convs_dir).unwrap();
        std::fs::create_dir_all(&mock_ws).unwrap();

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

        let uris_json = format!("[\"{}\"]", mock_ws.display());
        conn.execute(
            "INSERT INTO conversation_summaries VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "test-uuid-1234",
                "mock_agent",
                uris_json,
                "2026-11-07T00:00:00Z",
                5,
                "Testing session"
            ],
        )
        .unwrap();

        std::fs::write(convs_dir.join("test-uuid-1234.db"), b"").unwrap();

        let res =
            query_session_from_sources(&db_path, &convs_dir, &history_path, "mock_agent", &mock_ws);
        assert!(res.is_some());
        let session = res.unwrap();
        assert_eq!(session.conversation_id, "test-uuid-1234");
        assert_eq!(session.step_count, 5);

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_rusqlite_session_discovery_via_history_jsonl() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tkg-ctrl-test-session-hist-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock_ws = temp_dir.join("cli_project");
        let other_ws = temp_dir.join("other_project");
        let gemini_dir = temp_dir.join(".gemini/antigravity-cli");
        let convs_dir = gemini_dir.join("conversations");
        std::fs::create_dir_all(&convs_dir).unwrap();
        std::fs::create_dir_all(&mock_ws).unwrap();
        std::fs::create_dir_all(&other_ws).unwrap();

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
            "workspace": mock_ws.display().to_string(),
            "display": "Test prompt"
        });
        std::fs::write(&history_path, format!("{hist_entry}\n")).unwrap();

        std::fs::write(convs_dir.join("cli-uuid-5678.db"), b"").unwrap();

        let res =
            query_session_from_sources(&db_path, &convs_dir, &history_path, "copilot", &mock_ws);
        assert!(res.is_some());
        let session = res.unwrap();
        assert_eq!(session.conversation_id, "cli-uuid-5678");
        assert_eq!(session.step_count, 47);

        // Mismatched workspace should return None
        let res_none =
            query_session_from_sources(&db_path, &convs_dir, &history_path, "copilot", &other_ws);
        assert!(res_none.is_none());

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_rusqlite_session_discovery_ignores_zero_steps() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tkg-ctrl-test-session-zero-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock_ws = temp_dir.join("zero_project");
        let gemini_dir = temp_dir.join(".gemini/antigravity-cli");
        let convs_dir = gemini_dir.join("conversations");
        std::fs::create_dir_all(&convs_dir).unwrap();
        std::fs::create_dir_all(&mock_ws).unwrap();

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

        // 0 step conversation should not be eligible for resumption
        let uris_json = format!("[\"{}\"]", mock_ws.display());
        conn.execute(
            "INSERT INTO conversation_summaries VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "zero-uuid-9999",
                "copilot",
                uris_json,
                "2026-11-07T23:00:00Z",
                0,
                ""
            ],
        )
        .unwrap();

        std::fs::write(convs_dir.join("zero-uuid-9999.db"), b"").unwrap();

        let res =
            query_session_from_sources(&db_path, &convs_dir, &history_path, "copilot", &mock_ws);
        assert!(res.is_none());

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_rusqlite_session_discovery_subdirectory_matching() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tkg-ctrl-test-session-sub-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock_ws = temp_dir.join("root_project");
        let sub_ws = mock_ws.join("crates").join("my_crate");
        let gemini_dir = temp_dir.join(".gemini/antigravity-cli");
        let convs_dir = gemini_dir.join("conversations");
        std::fs::create_dir_all(&convs_dir).unwrap();
        std::fs::create_dir_all(&sub_ws).unwrap();

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
                "sub-uuid-7777",
                "coder",
                "",
                "2026-11-07T23:30:00Z",
                12,
                "Subdirectory turn"
            ],
        )
        .unwrap();

        // Prompt was run from sub_ws (inside the workspace root)
        let hist_entry = serde_json::json!({
            "conversationId": "sub-uuid-7777",
            "workspace": sub_ws.display().to_string(),
            "display": "Subdirectory prompt"
        });
        std::fs::write(&history_path, format!("{hist_entry}\n")).unwrap();

        std::fs::write(convs_dir.join("sub-uuid-7777.db"), b"").unwrap();

        // Querying from workspace root should successfully match the session
        let res =
            query_session_from_sources(&db_path, &convs_dir, &history_path, "coder", &mock_ws);
        assert!(res.is_some());
        let session = res.unwrap();
        assert_eq!(session.conversation_id, "sub-uuid-7777");
        assert_eq!(session.step_count, 12);

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_opencode_session_discovery() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tkg-ctrl-test-opencode-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock_ws = temp_dir.join("opencode_workspace");
        let opencode_sessions = mock_ws.join(".opencode").join("sessions");
        std::fs::create_dir_all(&opencode_sessions).unwrap();

        let session_data = serde_json::json!({
            "id": "opencode-ses-1234",
            "agent": "coder",
            "updatedAt": "2026-11-08T18:00:00Z",
            "title": "Fix buffer overflow in parser",
            "messages": [
                {"role": "user", "content": "hello"},
                {"role": "assistant", "content": "hi"}
            ]
        });

        std::fs::write(
            opencode_sessions.join("ses_1234.json"),
            session_data.to_string(),
        )
        .unwrap();

        let provider = OpenCodeSessionProvider;
        let res = provider.find_agent_session("coder", &mock_ws);
        assert!(res.is_some());
        let sess = res.unwrap();
        assert_eq!(sess.conversation_id, "opencode-ses-1234");
        assert_eq!(sess.agent_name, "coder");
        assert_eq!(sess.step_count, 2);
        assert_eq!(sess.preview, "Fix buffer overflow in parser");

        // Different agent should not match
        let diff_res = provider.find_agent_session("reviewer", &mock_ws);
        assert!(diff_res.is_none());

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_generic_state_session_discovery() {
        let temp_dir = std::env::temp_dir().join(format!(
            "tkg-ctrl-test-generic-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock_ws = temp_dir.join("generic_workspace");
        let state_sessions = mock_ws.join(".tkg").join("state").join("sessions");
        std::fs::create_dir_all(&state_sessions).unwrap();

        let session_data = serde_json::json!({
            "conversation_id": "gen-session-999",
            "agent_name": "planner",
            "last_modified_time": "2026-11-08T19:00:00Z",
            "step_count": 5,
            "preview": "Architecture roadmap review"
        });

        std::fs::write(
            state_sessions.join("gen_999.json"),
            session_data.to_string(),
        )
        .unwrap();

        let provider = GenericSessionProvider;
        let res = provider.find_agent_session("planner", &mock_ws);
        assert!(res.is_some());
        let sess = res.unwrap();
        assert_eq!(sess.conversation_id, "gen-session-999");
        assert_eq!(sess.step_count, 5);
        assert_eq!(sess.preview, "Architecture roadmap review");

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
