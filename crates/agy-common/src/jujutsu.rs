//! Jujutsu (`jj`) workspace and isolation primitives.

use std::path::{Path, PathBuf};
use std::process::Command;
use crate::error::{AgyError, Result};
use crate::limits::DEFAULT_VCS_QUERY_TIMEOUT_SECS;
use crate::supervisor::SubprocessSupervisor;

/// Primitives for managing isolated Jujutsu workspaces.
#[derive(Debug, Clone, Copy)]
pub struct JujutsuWorkspace;

impl JujutsuWorkspace {
    /// Checks whether the target directory or its ancestors is a Jujutsu repository.
    #[must_use]
    pub fn is_jj_repo(workspace_root: &Path) -> bool {
        if workspace_root.join(".jj").exists() {
            return true;
        }
        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "root"]);
        cmd.current_dir(workspace_root);
        SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
            .is_ok_and(|out| out.status.success())
    }

    /// Returns the active workspace name (e.g., "default", "coder") for `workspace_root`.
    #[must_use]
    pub fn current_workspace_name(workspace_root: &Path) -> Option<String> {
        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "workspace", "root"]);
        cmd.current_dir(workspace_root);
        let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS).ok()?;
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
        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "workspace", "list"]);
        cmd.current_dir(workspace_root);
        let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);
        if let Ok(out) = output
            && out.status.success()
        {
            let list_str = String::from_utf8_lossy(&out.stdout);
            for line in list_str.lines() {
                if let Some(first) = line.split(':').next() {
                    let ws_id = first.split_whitespace().next().unwrap_or("").trim();
                    if ws_id == name {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Checks if an isolated workspace has zero uncommitted changes.
    #[must_use]
    #[allow(clippy::print_stderr, reason = "Advisory notice when jj binary is absent")]
    pub fn is_workspace_clean(ws_path: &Path) -> bool {
        if !is_jj_in_path() {
            eprintln!("[agy-common] Advisory: 'jj' (Jujutsu) CLI not found in PATH; cannot check workspace clean state.");
            return false;
        }

        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "diff", "--summary"]);
        cmd.current_dir(ws_path);
        let Ok(out) = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS) else {
            return false;
        };

        if !out.status.success() {
            return false;
        }

        let project = crate::project::ProjectConfig::detect_or_load(ws_path);
        let stdout = String::from_utf8_lossy(&out.stdout);
        !stdout.lines().any(|l| {
            let trimmed = l.trim();
            if trimmed.is_empty() {
                return false;
            }
            let path_to_check = if let Some((_op, rest)) = trimmed.split_once(' ') {
                rest.trim()
            } else {
                trimmed
            };
            !project.is_ignored_or_cache_path(path_to_check)
        })
    }

    /// Ensures an isolated Jujutsu workspace exists for the specified agent.
    ///
    /// Creates `.agy/workspaces/<agent>` if it does not already exist.
    /// If it already exists and is clean, automatically synchronizes it by branching
    /// a new working copy commit off the latest `default@` revision.
    /// Links `.agy/workspaces/<agent>/target` to `<workspace_root>/target` to share build artifacts.
    /// Returns the canonical path to the isolated workspace.
    ///
    /// # Errors
    /// Returns `AgyError` if Jujutsu is not installed, not initialized, or workspace creation fails.
    #[allow(clippy::print_stderr, reason = "Diagnostic logging for subprocess and symlink errors")]
    pub fn ensure_agent_workspace(workspace_root: &Path, agent: &str) -> Result<PathBuf> {
        let repo_root = crate::resolver::WorkspaceResolver::find_root(Some(workspace_root))
            .unwrap_or_else(|_| workspace_root.to_path_buf());

        if !is_jj_in_path() {
            eprintln!("[agy-common] Advisory: 'jj' (Jujutsu) CLI not found in PATH; cannot initialize isolated agent workspace.");
            return Err(AgyError::Process("Jujutsu 'jj' CLI binary not found in PATH".to_owned()));
        }

        if !Self::is_jj_repo(&repo_root) {
            if repo_root.join(".git").exists() {
                eprintln!(
                    "[agy-common] Advisory: Target workspace '{}' is a Git repository without Jujutsu colocation.\n\
                     Run 'jj git init --colocate' in the repository root to enable isolated agent workspaces.",
                    repo_root.display()
                );
                return Err(AgyError::Workspace(format!(
                    "Target workspace '{}' is a Git repository without Jujutsu colocation. \
                     Run 'jj git init --colocate' in the repository root to enable isolated agent workspaces.",
                    repo_root.display()
                )));
            }
            eprintln!(
                "[agy-common] Advisory: Target workspace '{}' is not a Jujutsu repository (.jj not found).\n\
                 Run 'jj git init --colocate' (for Git projects) or 'jj init' to enable isolated agent workspaces.",
                repo_root.display()
            );
            return Err(AgyError::Workspace(format!(
                "Target workspace '{}' is not a Jujutsu repository (.jj not found). \
                 Run 'jj git init --colocate' or 'jj init' to enable isolated agent workspaces.",
                repo_root.display()
            )));
        }

        let paths = crate::state::AgyStatePaths::new(&repo_root);
        let workspaces_dir = paths.workspaces_dir();
        std::fs::create_dir_all(&workspaces_dir)?;
        let agent_ws_path = paths.agent_workspace(agent);

        if !Self::workspace_exists(&repo_root, agent) {
            let dest_str = agent_ws_path.to_str().unwrap_or(agent);
            let mut cmd = Command::new("jj");
            cmd.args(["--color=never", "workspace", "add", "--name", agent, dest_str]);
            cmd.current_dir(&repo_root);
            let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
                .map_err(|e| AgyError::Process(format!("Failed to execute 'jj workspace add': {e}")))?;

            if !output.status.success() {
                return Err(AgyError::Process(format!(
                    "Command 'jj workspace add --name {agent} {}' exited with non-zero status",
                    agent_ws_path.display()
                )));
            }
        } else if Self::is_workspace_clean(&agent_ws_path) {
            // Rebase clean workspace onto latest default@ commit to eliminate split-brain drift
            let mut cmd = Command::new("jj");
            cmd.args(["--color=never", "new", "default@", "--no-edit"]);
            cmd.current_dir(&agent_ws_path);
            match SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS) {
                Ok(out) if !out.status.success() => {
                    eprintln!(
                        "[agy-common] warning: 'jj new default@' in {} exited with non-zero status: {}",
                        agent_ws_path.display(),
                        out.status
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[agy-common] warning: failed to execute 'jj new default@' in {}: {e}",
                        agent_ws_path.display()
                    );
                }
                _ => {}
            }
        }

        // Share declared build caches from project descriptor to avoid cold-start compilation overhead
        let project = crate::project::ProjectConfig::detect_or_load(&repo_root);
        for cache_name in project.all_ignored_or_cache_paths() {
            let root_cache = repo_root.join(&cache_name);
            let ws_cache = agent_ws_path.join(&cache_name);
            if !ws_cache.exists() && root_cache.is_dir() {
                #[cfg(unix)]
                {
                    if let Some(parent) = ws_cache.parent() {
                        drop(std::fs::create_dir_all(parent));
                    }
                    if let Err(e) = std::os::unix::fs::symlink(&root_cache, &ws_cache)
                        && e.kind() != std::io::ErrorKind::AlreadyExists
                    {
                        eprintln!(
                            "[agy-common] warning: failed to symlink cache directory from {} to {}: {e}",
                            root_cache.display(),
                            ws_cache.display()
                        );
                    }
                }
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
    #[allow(clippy::print_stderr, reason = "Diagnostic logging for subprocess errors")]
    pub fn forget_agent_workspace(workspace_root: &Path, agent: &str) -> Result<()> {
        let ws_path = crate::state::AgyStatePaths::new(workspace_root).agent_workspace(agent);
        if Self::workspace_exists(workspace_root, agent) {
            let status = Command::new("jj")
                .args(["--color=never", "workspace", "forget", agent])
                .current_dir(workspace_root)
                .status()
                .map_err(|e| AgyError::Process(format!("Failed to execute 'jj workspace forget {agent}': {e}")))?;
            if !status.success() {
                eprintln!(
                    "[agy-common] warning: 'jj workspace forget {agent}' exited with non-zero status: {status}"
                );
            }
        }
        if ws_path.exists() {
            std::fs::remove_dir_all(&ws_path).map_err(AgyError::Io)?;
        }
        Ok(())
    }
}

fn is_jj_in_path() -> bool {
    if let Some(paths) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&paths) {
            if p.join("jj").is_file() {
                return true;
            }
        }
    }
    false
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
        let _clean = JujutsuWorkspace::is_workspace_clean(&root);
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
