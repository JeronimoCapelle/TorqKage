//! Session state handoff recording (`Stop`) and wake-up injection (`SessionStart`).

use std::path::{Path, PathBuf};
use std::process::Command;
use agy_common::{
    HandoffRecord, InjectStep, PipelineHandoff, SessionStartInput, SessionStartResponse, StopInput,
    StopResponse, WorkspaceResolver,
};
use crate::stdin::read_stdin_payload;

/// Executes the `record-handoff` hook (`Stop`).
///
/// Captures working copy changes and active agent metadata, writing a Markdown summary
/// to `.agy/state/last_handoff.md` and structured JSON to `.agy/state/handoff.json`.
/// Emits `{}` on `stdout` permitting loop termination.
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

    let record = HandoffRecord::new(&agent_name, modified_files.clone(), &status);

    let paths = agy_common::AgyStatePaths::new(&root);
    if std::fs::create_dir_all(paths.state_dir()).is_ok() {
        let handoff_path = paths.handoff_md_file();
        drop(std::fs::write(&handoff_path, record.to_markdown()));

        let pipeline_handoff = PipelineHandoff::new(
            &agent_name,
            None,
            format!("Session completed by agent '{agent_name}' with status '{status}'."),
            modified_files,
            Vec::new(),
            status,
        );
        if let Ok(json_str) = serde_json::to_string_pretty(&pipeline_handoff) {
            drop(std::fs::write(paths.handoff_json_file(), json_str));
        }
    }

    let resp = StopResponse::allow();
    if let Ok(json) = serde_json::to_string(&resp) {
        println!("{json}");
    }
}

/// Executes the `inject-wake` hook (`SessionStart`).
///
/// Reads `.agy/state/handoff.json` (or `.agy/state/last_handoff.md`) if present and emits a `systemMessage` step
/// providing immediate continuity context for the starting conversation session.
pub(crate) fn execute_inject_wake() {
    let input: SessionStartInput = read_stdin_payload();

    let root = resolve_workspace_root(input.common.workspace_paths.first());
    let paths = agy_common::AgyStatePaths::new(&root);
    let json_handoff = paths.handoff_json_file();
    let md_handoff = paths.handoff_md_file();

    let resp = if json_handoff.exists()
        && let Ok(content) = std::fs::read_to_string(&json_handoff)
        && let Ok(handoff) = serde_json::from_str::<PipelineHandoff>(&content)
    {
        SessionStartResponse {
            inject_steps: vec![InjectStep::system(handoff.to_injection_message())],
        }
    } else if md_handoff.exists()
        && let Ok(content) = std::fs::read_to_string(&md_handoff)
        && let Some(record) = HandoffRecord::from_markdown(&content)
    {
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
    let raw_output = if agy_common::JujutsuWorkspace::is_jj_repo(root) {
        Command::new("jj")
            .args(["--color=never", "diff", "--summary"])
            .current_dir(root)
            .output()
    } else {
        Command::new("git")
            .args(["status", "-s"])
            .current_dir(root)
            .output()
    };

    let mut files = Vec::new();
    if let Ok(out) = raw_output
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines().take(10) {
            let trimmed = line.trim();
            if trimmed == "A target" || trimmed == "M target" {
                continue;
            }
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
