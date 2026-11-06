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
    let config = crate::config::HooksConfig::load(&root);

    if !config.stop.enabled || !config.stop.record_handoff {
        let resp = StopResponse::allow();
        if let Ok(json) = serde_json::to_string(&resp) {
            println!("{json}");
        }
        return;
    }

    let modified_files = collect_modified_files(&root, config.stop.max_modified_files);
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
#[allow(
    clippy::format_push_string,
    clippy::too_many_lines,
    reason = "Building injection prompt message and evaluating declarative wake context"
)]
pub(crate) fn execute_inject_wake() {
    let input: SessionStartInput = read_stdin_payload();

    let root = resolve_workspace_root(input.common.workspace_paths.first());
    let config = crate::config::HooksConfig::load(&root);

    if !config.session_start.enabled {
        let resp = SessionStartResponse::default();
        if let Ok(json) = serde_json::to_string(&resp) {
            println!("{json}");
        }
        return;
    }

    let paths = agy_common::AgyStatePaths::new(&root);
    let json_handoff = paths.handoff_json_file();
    let md_handoff = paths.handoff_md_file();
    let project = agy_common::ProjectConfig::detect_or_load(&root);
    let tool_name = project.docs.tool_name.as_deref().unwrap_or("query_project_spec");

    let mut resp = if config.session_start.inject_wake_state
        && json_handoff.exists()
        && let Ok(content) = std::fs::read_to_string(&json_handoff)
        && let Ok(handoff) = serde_json::from_str::<PipelineHandoff>(&content)
    {
        let mut msg = handoff.to_injection_message();
        if config.session_start.inject_documentation && !project.docs.paths.is_empty() {
            msg.push_str(&format!(
                "\n\n[DOCUMENTATION]: Available in [{}]. Use '{tool_name}' to search specifications.",
                project.docs.paths.join(", ")
            ));
        }
        SessionStartResponse {
            inject_steps: vec![InjectStep::system(msg)],
        }
    } else if config.session_start.inject_wake_state
        && md_handoff.exists()
        && let Ok(content) = std::fs::read_to_string(&md_handoff)
        && let Some(record) = HandoffRecord::from_markdown(&content)
    {
        let files_str = if record.modified_files.is_empty() {
            "Working tree clean.".to_owned()
        } else {
            format!("Modified files: {}.", record.modified_files.join(", "))
        };

        let mut msg = format!(
            "[WAKE-UP STATE]: Last session was with {}. {}. Status: {}.",
            record.previous_agent, files_str, record.status
        );
        if config.session_start.inject_documentation && !project.docs.paths.is_empty() {
            msg.push_str(&format!(
                " Documentation paths: [{}]. Use '{tool_name}' to search.",
                project.docs.paths.join(", ")
            ));
        }

        SessionStartResponse {
            inject_steps: vec![InjectStep::system(msg)],
        }
    } else if config.session_start.inject_documentation && !project.docs.paths.is_empty() {
        let msg = format!(
            "[PROJECT CONTEXT]: Documentation and specifications are located in: [{}]. Use '{tool_name}' to search them.",
            project.docs.paths.join(", ")
        );
        SessionStartResponse {
            inject_steps: vec![InjectStep::system(msg)],
        }
    } else {
        SessionStartResponse::default()
    };

    // Resolve active agent identity to match targeted injections
    let active_agent = std::env::var("AGY_AGENT")
        .or_else(|_| std::env::var("MCP_SCOPE"))
        .ok()
        .or_else(|| {
            let json_path = paths.active_agent_file();
            if json_path.is_file()
                && let Ok(content) = std::fs::read_to_string(&json_path)
                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(name) = val.get("agent").and_then(serde_json::Value::as_str)
            {
                return Some(name.to_owned());
            }
            let txt_path = paths.state_dir().join("active_agent");
            if txt_path.is_file()
                && let Ok(content) = std::fs::read_to_string(&txt_path)
            {
                let name = content.trim();
                if !name.is_empty() {
                    return Some(name.to_owned());
                }
            }
            None
        })
        .unwrap_or_default();

    // Evaluate declarative custom wake injections from hooks.toml
    for injection in &config.session_start.injections {
        if !crate::config::matches_pattern(&active_agent, &injection.matcher) {
            continue;
        }

        if let Some(binary) = injection.command.first() {
            let args = injection.command.get(1..).unwrap_or(&[]);
            let mut cmd = Command::new(binary);
            cmd.args(args);
            cmd.current_dir(&root);

            if let Ok(out) = agy_common::SubprocessSupervisor::run_supervised(cmd, injection.timeout_seconds) {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
                let raw = if stdout.is_empty() { stderr } else { stdout };

                if !raw.is_empty() {
                    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
                    let truncated: Vec<&str> = lines.iter().take(injection.max_lines).copied().collect();
                    let mut content = truncated.join("\n");
                    if lines.len() > injection.max_lines {
                        content.push_str(&format!(
                            "\n[... truncated {} additional lines ...]",
                            lines.len().saturating_sub(injection.max_lines)
                        ));
                    }
                    let msg = format!("{}{content}", injection.prefix);
                    resp.inject_steps.push(InjectStep::system(msg));
                } else if let Some(ref fallback) = injection.fallback {
                    let msg = format!("{}{fallback}", injection.prefix);
                    resp.inject_steps.push(InjectStep::system(msg));
                }
            }
        }
    }

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

fn collect_modified_files(root: &Path, max_files: usize) -> Vec<String> {
    let project = agy_common::ProjectConfig::detect_or_load(root);
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
        for line in s.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (path, file_str) = if let Some((_, p)) = trimmed.split_once(' ') {
                (p.trim(), p.trim().to_owned())
            } else {
                (trimmed, trimmed.to_owned())
            };
            if project.is_ignored_or_cache_path(path) {
                continue;
            }
            files.push(file_str);
            if files.len() >= max_files {
                break;
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
