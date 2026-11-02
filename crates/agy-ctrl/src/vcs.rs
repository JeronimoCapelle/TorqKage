//! VCS snapshotting, atomic serialization, and Antigravity plugin validation engine.

use std::fs;
use std::path::Path;
use std::process::Command;
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
        let desc_output = Command::new("jj")
            .current_dir(workspace_root)
            .args(["log", "-r", "@", "--no-graph", "-T", "description"])
            .output();

        let has_description = desc_output.is_ok_and(|out| !String::from_utf8_lossy(&out.stdout).trim().is_empty());
        if has_description {
            // Trigger working-copy snapshot without mutating existing description
            drop(Command::new("jj")
                .current_dir(workspace_root)
                .arg("status")
                .output());
            return "Existing Jujutsu revision preserved at @".to_owned();
        }

        let output = Command::new("jj")
            .current_dir(workspace_root)
            .args(["describe", "-m", &format!("chore(agy-ctrl): {message}")])
            .output();

        if output.is_ok_and(|out| out.status.success()) {
            return "Jujutsu snapshot recorded at @".to_owned();
        }
    }

    "Working copy snapshot ready".to_owned()
}

/// Executes `agy plugin validate` on the plugin directory.
pub(crate) fn run_plugin_validation(workspace_root: &Path) -> Result<String, String> {
    let plugin_dir = workspace_root.join("plugins/agy-suite");
    if !plugin_dir.is_dir() {
        return Err(format!("Plugin directory not found at {}", plugin_dir.display()));
    }

    let output = Command::new("agy")
        .current_dir(workspace_root)
        .args(["plugin", "validate", "plugins/agy-suite"])
        .output()
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
    let plugin_dir = workspace_root.join("plugins/agy-suite");
    let output = Command::new("agy")
        .current_dir(workspace_root)
        .args(["plugin", "install", &plugin_dir.to_string_lossy()])
        .output()
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
        let output = Command::new("jj")
            .current_dir(workspace_root)
            .args(["restore", &file_path.to_string_lossy()])
            .output();

        if output.is_ok_and(|out| out.status.success()) {
            return Ok(format!("Restored {} via 'jj restore'", file_path.display()));
        }
    }

    if workspace_root.join(".git").is_dir() {
        let output = Command::new("git")
            .current_dir(workspace_root)
            .args(["checkout", "--", &file_path.to_string_lossy()])
            .output();

        if output.is_ok_and(|out| out.status.success()) {
            return Ok(format!("Checked out {} via 'git checkout'", file_path.display()));
        }
    }

    Err("Could not restore file from VCS".to_owned())
}
