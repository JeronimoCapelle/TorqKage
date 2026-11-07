//! Project-specific conversation session reader and discovery engine for Antigravity.
//!
//! # Runtime Requirements
//! This module queries session metadata from `~/.gemini/antigravity-cli/conversation_summaries.db`.
//! It requires the external `sqlite3` CLI utility installed in `$PATH` to execute JSON queries.
//! If `sqlite3` is absent, session discovery gracefully returns `None`.

use std::{
    path::{Path, PathBuf},
    process::Command,
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

/// Discovers the latest resumable session for a given agent in the given workspace.
///
/// Returns `None` if `sqlite3` is not found in PATH or if the session database does not exist.
#[must_use]
pub(crate) fn find_agent_session_for_workspace(
    agent_name: &str,
    workspace_root: &Path,
) -> Option<SessionSummary> {
    if which::which("sqlite3").is_err() {
        eprintln!(
            "[agy-ctrl] Advisory: 'sqlite3' CLI not found in PATH; past conversation session discovery disabled."
        );
        return None;
    }

    let home = std::env::var("HOME").ok()?;
    let home_path = PathBuf::from(&home);
    let db_path = home_path.join(".gemini/antigravity-cli/conversation_summaries.db");
    if !db_path.is_file() {
        return None;
    }

    let ws_canonical = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let ws_lossy = ws_canonical.to_string_lossy();

    // Query SQLite database for latest matching conversation with at least 1 step
    let sql = format!(
        "SELECT conversation_id, agent_name, workspace_uris, last_modified_time, step_count, preview \
         FROM conversation_summaries \
         WHERE agent_name = '{}' AND workspace_uris LIKE '%{}%' AND step_count > 0 \
         ORDER BY last_modified_time DESC LIMIT 5;",
        agent_name.replace('\'', "''"),
        ws_lossy.replace('\'', "''")
    );

    let output = Command::new("sqlite3")
        .arg(&db_path)
        .arg("-json")
        .arg(&sql)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let items: Vec<SessionSummary> = serde_json::from_slice(&output.stdout).ok()?;
    let convs_dir = home_path.join(".gemini/antigravity-cli/conversations");

    // Only return a session if its backing SQLite database actually exists on disk
    for item in items {
        let conv_file = convs_dir.join(format!("{}.db", item.conversation_id));
        if conv_file.is_file() {
            return Some(item);
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
            agent_name: "coder".to_owned(),
            workspace_uris: "[]".to_owned(),
            last_modified_time: "2026-11-02".to_owned(),
            step_count: 10,
            preview: "Implement feature".to_owned(),
        };
        assert_eq!(summary.short_id(), "6030b881");
    }
}
