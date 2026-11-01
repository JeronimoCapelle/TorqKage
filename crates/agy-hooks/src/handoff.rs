//! Session state handoff recording (`Stop`) and wake-up injection (`SessionStart`).

use std::path::{Path, PathBuf};
use std::process::Command;
use agy_common::{
    HandoffRecord, InjectStep, SessionStartInput, SessionStartResponse, StopInput, StopResponse,
    WorkspaceResolver,
};
use crate::stdin::read_stdin_payload;

/// Executes the `record-handoff` hook (`Stop`).
///
/// Captures working copy changes and active agent metadata, writing a Markdown summary
/// to `.pipeline/last_handoff.md`. Emits `{}` on `stdout` permitting loop termination.
pub(crate) fn execute_record_handoff() {
    let input: StopInput = read_stdin_payload();

    let root = resolve_workspace_root(input.common.workspace_paths.first());

    let modified_files = collect_modified_files(&root);
    let agent_name = std::env::var("AGY_AGENT")
        .or_else(|_| input.common.model_name.clone().ok_or(()))
        .unwrap_or_else(|()| "agent".to_owned());

    let status = if modified_files.is_empty() {
        "Clean".to_owned()
    } else {
        "WIP".to_owned()
    };

    let record = HandoffRecord::new(agent_name, modified_files, status);

    let pipeline_dir = root.join(".pipeline");
    if std::fs::create_dir_all(&pipeline_dir).is_ok() {
        let handoff_path = pipeline_dir.join("last_handoff.md");
        drop(std::fs::write(&handoff_path, record.to_markdown()));
    }

    let resp = StopResponse::allow();
    if let Ok(json) = serde_json::to_string(&resp) {
        println!("{json}");
    }
}

/// Executes the `inject-wake` hook (`SessionStart`).
///
/// Reads `.pipeline/last_handoff.md` if present and emits a `systemMessage` step
/// providing immediate continuity context for the starting conversation session.
pub(crate) fn execute_inject_wake() {
    let input: SessionStartInput = read_stdin_payload();

    let root = resolve_workspace_root(input.common.workspace_paths.first());
    let handoff_path = root.join(".pipeline").join("last_handoff.md");

    let resp = if handoff_path.exists() {
        std::fs::read_to_string(&handoff_path).map_or_else(
            |_| SessionStartResponse::default(),
            |content| {
                if let Some(record) = HandoffRecord::from_markdown(&content) {
                    let files_str = if record.modified_files.is_empty() {
                        "Working tree clean.".to_owned()
                    } else {
                        format!("Modified files: {}.", record.modified_files.join(", "))
                    };

                    let msg = format!(
                        "[WAKE-UP STATE]: Last session was with {}. {}. Status: {}.",
                        record.previous_agent, files_str, record.status
                    );

                    SessionStartResponse {
                        inject_steps: vec![InjectStep::system(msg)],
                    }
                } else {
                    SessionStartResponse::default()
                }
            },
        )
    } else {
        SessionStartResponse::default()
    };

    if let Ok(json) = serde_json::to_string(&resp) {
        println!("{json}");
    }
}

fn resolve_workspace_root(candidate: Option<&PathBuf>) -> PathBuf {
    if let Some(first_path) = candidate
        && first_path.exists()
    {
        return first_path.clone();
    }

    WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."))
}

fn collect_modified_files(root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["status", "-s"])
        .current_dir(root)
        .output();

    let mut files = Vec::new();
    if let Ok(out) = output
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines().take(10) {
            let trimmed = line.trim();
            if let Some((_, path)) = trimmed.split_once(' ') {
                files.push(path.trim().to_owned());
            } else if !trimmed.is_empty() {
                files.push(trimmed.to_owned());
            }
        }
    }
    files
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_empty_wake_up_produces_empty_steps() {
        let resp = SessionStartResponse::default();
        let json = serde_json::to_string(&resp).expect("serialize failed");
        assert_eq!(json, r#"{"injectSteps":[]}"#);
    }
}
