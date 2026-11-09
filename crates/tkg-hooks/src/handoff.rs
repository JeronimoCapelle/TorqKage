//! Session state handoff recording (`Stop`) and wake-up injection (`SessionStart`).

use std::{
    fmt::Write,
    path::{Path, PathBuf},
    process::Command,
};

use tkg_common::{
    InjectStep, SessionStartInput, SessionStartResponse, StopInput, StopResponse, WorkspaceResolver,
};

use crate::stdin::read_stdin_payload;

/// Executes the `record-handoff` hook (`Stop`).
///
/// Captures working copy changes and active agent metadata, writing a Markdown summary
/// to `.tkg/state/last_handoff.md` and structured JSON to `.tkg/state/handoff.json`.
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

    let agent_name = tkg_common::resolve_active_agent(&root)
        .or_else(|| input.common.model_name.clone())
        .unwrap_or_else(|| "agent".to_owned());

    let summary = format!("Session completed by agent '{agent_name}'.");
    drop(tkg_common::handoff::record_session_handoff(
        &root,
        &agent_name,
        None,
        &summary,
        &[],
        "Auto",
        config.stop.max_modified_files,
    ));

    let resp = StopResponse::allow();
    if let Ok(json) = serde_json::to_string(&resp) {
        println!("{json}");
    }
}

/// Executes the `inject-wake` hook (`SessionStart`).
///
/// Reads `.tkg/state/handoff.json` (or `.tkg/state/last_handoff.md`) if present and emits a `systemMessage` step
/// providing immediate continuity context for the starting conversation session.
fn resolve_handoff_step(
    config: &crate::config::SessionStartConfig,
    root: &Path,
    project: &tkg_common::ProjectConfig,
) -> Option<InjectStep> {
    let tool_hint = project.docs.tool_name.as_deref().map_or_else(
        || "Search documentation via configured documentation tools.".to_owned(),
        |name| format!("Use '{name}' to search specifications."),
    );

    if config.inject_wake_state
        && let Some(mut msg) = tkg_common::handoff::read_last_handoff_message(root)
    {
        if config.inject_documentation && !project.docs.paths.is_empty() {
            _ = write!(
                msg,
                "\n\n[DOCUMENTATION]: Available in [{}]. {tool_hint}",
                project.docs.paths.join(", ")
            );
        }
        return Some(InjectStep::system(msg));
    }

    if config.inject_documentation && !project.docs.paths.is_empty() {
        let msg = format!(
            "[PROJECT CONTEXT]: Documentation and specifications are located in: [{}]. {tool_hint}",
            project.docs.paths.join(", ")
        );
        return Some(InjectStep::system(msg));
    }

    None
}

fn resolve_active_agent(paths: &tkg_common::TkgStatePaths) -> String {
    tkg_common::resolve_active_agent(paths.workspace_root()).unwrap_or_default()
}

fn evaluate_custom_injections(
    root: &Path,
    active_agent: &str,
    injections: &[crate::config::WakeInjection],
    steps: &mut Vec<InjectStep>,
) {
    for injection in injections {
        if !crate::config::matches_pattern(active_agent, &injection.matcher) {
            continue;
        }

        if let Some(binary) = injection.command.first() {
            let args = injection.command.get(1..).unwrap_or(&[]);
            let mut cmd = Command::new(binary);
            cmd.args(args);
            cmd.current_dir(root);

            if let Ok(out) =
                tkg_common::SubprocessSupervisor::run_supervised(cmd, injection.timeout_seconds)
            {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
                let raw = if stdout.is_empty() { stderr } else { stdout };

                if !raw.is_empty() {
                    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
                    let truncated: Vec<&str> =
                        lines.iter().take(injection.max_lines).copied().collect();
                    let mut content = truncated.join("\n");
                    if lines.len() > injection.max_lines {
                        _ = write!(
                            content,
                            "\n[... truncated {} additional lines ...]",
                            lines.len().saturating_sub(injection.max_lines)
                        );
                    }
                    let msg = format!("{}{content}", injection.prefix);
                    steps.push(InjectStep::system(msg));
                } else if let Some(ref fallback) = injection.fallback {
                    let msg = format!("{}{fallback}", injection.prefix);
                    steps.push(InjectStep::system(msg));
                }
            }
        }
    }
}

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

    let paths = tkg_common::TkgStatePaths::new(&root);
    let project = tkg_common::ProjectConfig::detect_or_load(&root);

    let mut resp = SessionStartResponse::default();
    if let Some(step) = resolve_handoff_step(&config.session_start, &root, &project) {
        resp.inject_steps.push(step);
    }

    let active_agent = resolve_active_agent(&paths);
    evaluate_custom_injections(
        &root,
        &active_agent,
        &config.session_start.injections,
        &mut resp.inject_steps,
    );

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
