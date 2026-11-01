//! Bounded tool action execution engines.

pub(crate) mod cargo_json;
pub(crate) mod cli_manual;
pub(crate) mod file_read;
pub(crate) mod file_search;
pub(crate) mod file_write;
pub(crate) mod mpatch_edit;
pub(crate) mod parameterized_cmd;
pub(crate) mod static_cmd;

use std::path::Path;
use crate::models::{ToolAction, ToolDefinition};

/// Dispatches tool execution to the appropriate action handler.
///
/// Returns a tuple of `(output_text, is_error)`.
///
/// # Errors
/// Returns an error message if the tool parameters, pre-flight gates, or execution fails.
pub(crate) fn dispatch_action(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
) -> Result<(String, bool), String> {
    evaluate_pre_flight_guards(tool, workspace_root)?;

    match tool.action {
        ToolAction::FileRead => {
            let content = file_read::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::FileWrite => {
            let content = file_write::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::MpatchEdit => {
            let content = mpatch_edit::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::FileSearch => {
            let content = file_search::execute(tool, workspace_root, arguments)?;
            Ok((content, false))
        }

        ToolAction::StaticCommand => static_cmd::execute(tool, workspace_root),

        ToolAction::ParameterizedCommand => {
            parameterized_cmd::execute(tool, workspace_root, arguments)
        }

        ToolAction::CargoJsonDiagnostics => cargo_json::execute(tool, workspace_root),

        ToolAction::CliManual => Ok(cli_manual::execute(tool, arguments)),

        ToolAction::Unknown(_) => Err(format!(
            "Unknown or unsupported tool action for tool '{}'",
            tool.name
        )),
    }
}

fn evaluate_pre_flight_guards(tool: &ToolDefinition, workspace_root: &Path) -> Result<(), String> {
    if tool.require_git_clean {
        check_git_clean(workspace_root)?;
    }
    if tool.require_clippy_clean {
        check_clippy_clean(workspace_root)?;
    }
    if tool.require_tests_passed {
        check_tests_passed(workspace_root)?;
    }
    Ok(())
}

fn check_git_clean(workspace_root: &Path) -> Result<(), String> {
    let jj_out = std::process::Command::new("jj")
        .args(["diff", "--stat"])
        .current_dir(workspace_root)
        .output();

    if let Ok(out) = jj_out
        && out.status.success()
        && !out.stdout.is_empty()
    {
        return Err("Pre-flight gate failed: 'require_git_clean' is enabled, but repository has uncommitted modifications.".to_owned());
    }

    let git_out = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workspace_root)
        .output();

    if let Ok(gout) = git_out
        && gout.status.success()
        && !gout.stdout.is_empty()
    {
        return Err("Pre-flight gate failed: 'require_git_clean' is enabled, but repository has uncommitted modifications.".to_owned());
    }

    Ok(())
}

fn check_clippy_clean(workspace_root: &Path) -> Result<(), String> {
    let output = std::process::Command::new("cargo")
        .args(["clippy", "--all-targets", "--", "-D", "warnings"])
        .current_dir(workspace_root)
        .output()
        .map_err(|err| format!("Failed to spawn cargo clippy gate: {err}"))?;

    if !output.status.success() {
        return Err("Pre-flight gate failed: 'require_clippy_clean' is enabled, but cargo clippy reported warnings or errors.".to_owned());
    }
    Ok(())
}

fn check_tests_passed(workspace_root: &Path) -> Result<(), String> {
    let output = std::process::Command::new("cargo")
        .args(["test", "--no-fail-fast"])
        .current_dir(workspace_root)
        .output()
        .map_err(|err| format!("Failed to spawn cargo test gate: {err}"))?;

    if !output.status.success() {
        return Err("Pre-flight gate failed: 'require_tests_passed' is enabled, but test suites reported failures.".to_owned());
    }
    Ok(())
}
