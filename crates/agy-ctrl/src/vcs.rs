//! VCS snapshotting, atomic serialization, and Antigravity plugin validation engine.

use std::fs;
use std::path::Path;
use std::process::Command;
use agy_common::{DEFAULT_TOOL_TIMEOUT_SECS, DEFAULT_VCS_QUERY_TIMEOUT_SECS, SubprocessSupervisor};
use crate::model::AgentManifest;

/// Writes an agent manifest back to disk.
pub(crate) fn save_agent_to_disk(agent: &AgentManifest) -> Result<(), String> {
    let content = agent.serialize();
    fs::write(&agent.file_path, content)
        .map_err(|e| format!("Failed to write {}: {e}", agent.file_path.display()))
}

/// Takes a quick VCS snapshot before applying edits to ensure atomic recovery without clobbering user descriptions.
pub(crate) fn take_vcs_snapshot(workspace_root: &Path, message: &str) -> String {
    if workspace_root.join(".jj").is_dir() {
        // Query current description to avoid clobbering user's commit message
        let mut desc_cmd = Command::new("jj");
        desc_cmd.current_dir(workspace_root);
        desc_cmd.args(["log", "-r", "@", "--no-graph", "-T", "description"]);
        let desc_output = SubprocessSupervisor::run_supervised(desc_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);

        let has_description = desc_output.is_ok_and(|out| !String::from_utf8_lossy(&out.stdout).trim().is_empty());
        if has_description {
            // Trigger working-copy snapshot without mutating existing description
            let mut status_cmd = Command::new("jj");
            status_cmd.current_dir(workspace_root);
            status_cmd.arg("status");
            drop(SubprocessSupervisor::run_supervised(status_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS));
            return "Existing Jujutsu revision preserved at @".to_owned();
        }

        let mut snapshot_cmd = Command::new("jj");
        snapshot_cmd.current_dir(workspace_root);
        snapshot_cmd.args(["describe", "-m", &format!("chore(agy-ctrl): {message}")]);
        let output = SubprocessSupervisor::run_supervised(snapshot_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);

        if output.is_ok_and(|out| out.status.success()) {
            return "Jujutsu snapshot recorded at @".to_owned();
        }
    }

    "Working copy snapshot ready".to_owned()
}

fn resolve_plugin_directory(workspace_root: &Path) -> Option<std::path::PathBuf> {
    let local = workspace_root.join("plugins/agy-suite");
    if local.is_dir() {
        return Some(local);
    }
    if let Ok(home) = std::env::var("HOME") {
        let installed = std::path::PathBuf::from(home).join(".gemini/config/plugins/agy-suite");
        if installed.is_dir() {
            return Some(installed);
        }
    }
    None
}

/// Executes `agy plugin validate` on the plugin directory.
pub(crate) fn run_plugin_validation(workspace_root: &Path) -> Result<String, String> {
    let plugin_dir = resolve_plugin_directory(workspace_root)
        .ok_or_else(|| "Plugin directory not found in workspace or ~/.gemini/config/plugins/agy-suite".to_owned())?;

    let mut cmd = Command::new("agy");
    cmd.current_dir(workspace_root);
    cmd.args(["plugin", "validate", &plugin_dir.to_string_lossy()]);
    let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_TOOL_TIMEOUT_SECS)
        .map_err(|e| format!("Failed to execute 'agy plugin validate': {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let combined = format!("{stdout}\n{stderr}").trim().to_owned();

    if output.status.success() && !combined.contains("failed") && !combined.contains("Error") {
        Ok(combined)
    } else {
        Err(if combined.is_empty() {
            "Validation failed with non-zero exit code".to_owned()
        } else {
            combined
        })
    }
}

/// Reinstalls the validated plugin into `~/.gemini/config/plugins/agy-suite`.
pub(crate) fn run_plugin_install(workspace_root: &Path) -> Result<String, String> {
    let plugin_dir = resolve_plugin_directory(workspace_root)
        .ok_or_else(|| "Plugin directory not found in workspace or ~/.gemini/config/plugins/agy-suite".to_owned())?;

    let mut cmd = Command::new("agy");
    cmd.current_dir(workspace_root);
    cmd.args(["plugin", "install", &plugin_dir.to_string_lossy()]);
    let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_TOOL_TIMEOUT_SECS)
        .map_err(|e| format!("Failed to execute 'agy plugin install': {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(format!("Install failed:\n{stdout}\n{stderr}"))
    }
}

/// Reverts changes to an agent file using VCS or reloading original disk state.
pub(crate) fn rollback_agent_file(workspace_root: &Path, file_path: &Path) -> Result<String, String> {
    if workspace_root.join(".jj").is_dir() {
        let mut cmd = Command::new("jj");
        cmd.current_dir(workspace_root);
        cmd.args(["restore", &file_path.to_string_lossy()]);
        let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);

        if output.is_ok_and(|out| out.status.success()) {
            return Ok(format!("Restored {} via 'jj restore'", file_path.display()));
        }
    }

    if workspace_root.join(".git").is_dir() {
        let mut cmd = Command::new("git");
        cmd.current_dir(workspace_root);
        cmd.args(["checkout", "--", &file_path.to_string_lossy()]);
        let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);

        if output.is_ok_and(|out| out.status.success()) {
            return Ok(format!("Checked out {} via 'git checkout'", file_path.display()));
        }
    }

    Err("Could not restore file from VCS".to_owned())
}
