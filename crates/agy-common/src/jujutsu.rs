//! Jujutsu (`jj`) workspace and isolation primitives.

use std::path::{Path, PathBuf};
use std::process::Command;
use crate::error::{AgyError, Result};

/// Primitives for managing isolated Jujutsu workspaces.
#[derive(Debug, Clone, Copy)]
pub struct JujutsuWorkspace;

impl JujutsuWorkspace {
    /// Checks whether the target directory or its ancestors is a Jujutsu repository.
    #[must_use]
    pub fn is_jj_repo(workspace_root: &Path) -> bool {
        workspace_root.join(".jj").exists()
            || Command::new("jj")
                .args(["--color=never", "root"])
                .current_dir(workspace_root)
                .output()
                .is_ok_and(|out| out.status.success())
    }

    /// Returns the active workspace name (e.g., "default", "coder") for `workspace_root`.
    #[must_use]
    pub fn current_workspace_name(workspace_root: &Path) -> Option<String> {
        let output = Command::new("jj")
            .args(["--color=never", "workspace", "root"])
            .current_dir(workspace_root)
            .output()
            .ok()?;
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let p = Path::new(&path_str);
            return p.file_name().and_then(|n| n.to_str()).map(ToOwned::to_owned);
        }
        None
    }

    /// Checks if a workspace with the given name already exists in the repo.
    #[must_use]
    pub fn workspace_exists(workspace_root: &Path, name: &str) -> bool {
        let output = Command::new("jj")
            .args(["--color=never", "workspace", "list"])
            .current_dir(workspace_root)
            .output();
        if let Ok(out) = output
            && out.status.success()
        {
            let list_str = String::from_utf8_lossy(&out.stdout);
            for line in list_str.lines() {
                if let Some(first) = line.split(':').next()
                    && first.trim() == name
                {
                    return true;
                }
            }
        }
        false
    }

    /// Ensures an isolated Jujutsu workspace exists for the specified agent.
    ///
    /// Creates `.workspaces/<agent>` if it does not already exist.
    /// Returns the canonical path to the isolated workspace.
    ///
    /// # Errors
    /// Returns `AgyError` if the filesystem or `jj workspace add` fails.
    pub fn ensure_agent_workspace(workspace_root: &Path, agent: &str) -> Result<PathBuf> {
        let workspaces_dir = workspace_root.join(".workspaces");
        std::fs::create_dir_all(&workspaces_dir)?;
        let agent_ws_path = workspaces_dir.join(agent);

        if !Self::workspace_exists(workspace_root, agent) {
            let dest_str = agent_ws_path.to_str().unwrap_or(agent);
            let status = Command::new("jj")
                .args(["--color=never", "workspace", "add", "--name", agent, dest_str])
                .current_dir(workspace_root)
                .status()
                .map_err(|e| AgyError::Process(format!("Failed to execute 'jj workspace add': {e}")))?;

            if !status.success() {
                return Err(AgyError::Process(format!(
                    "Command 'jj workspace add --name {agent} {}' exited with non-zero status",
                    agent_ws_path.display()
                )));
            }
        }

        if agent_ws_path.is_dir() {
            agent_ws_path.canonicalize().map_err(AgyError::Io)
        } else {
            Err(AgyError::Workspace(format!(
                "Workspace directory '{}' was not found after creation",
                agent_ws_path.display()
            )))
        }
    }

    /// Forgets an agent's workspace and deletes its directory from disk.
    ///
    /// # Errors
    /// Returns `AgyError` if workspace forget fails.
    pub fn forget_agent_workspace(workspace_root: &Path, agent: &str) -> Result<()> {
        let agent_ws_path = workspace_root.join(".workspaces").join(agent);
        if Self::workspace_exists(workspace_root, agent) {
            drop(Command::new("jj")
                .args(["--color=never", "workspace", "forget", agent])
                .current_dir(workspace_root)
                .status());
        }
        if agent_ws_path.exists() {
            drop(std::fs::remove_dir_all(&agent_ws_path));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_is_jj_repo_on_current_workspace() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert!(JujutsuWorkspace::is_jj_repo(&root));
    }

    #[test]
    fn test_workspace_exists_default() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert!(JujutsuWorkspace::workspace_exists(&root, "default"));
        assert!(!JujutsuWorkspace::workspace_exists(&root, "non_existent_workspace_12345"));
    }
}
