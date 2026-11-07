//! Dynamic repository and workspace root resolution.

use std::path::{Path, PathBuf};

use crate::error::{AgyError, Result};

/// Resolves the canonical repository or workspace root.
#[derive(Debug, Clone)]
pub struct WorkspaceResolver;

impl WorkspaceResolver {
    /// Discovers the canonical repository root by walking up from `start_dir`.
    ///
    /// Correctly distinguishes primary repositories from Jujutsu secondary workspaces
    /// (e.g. `.agy/workspaces/<agent>`) by resolving pointer files in `.jj/repo`.
    ///
    /// If `start_dir` is omitted and the process was launched inside an Antigravity plugin
    /// installation directory (`PLUGIN_ROOT`), it automatically recovers the caller's actual
    /// workspace root from `$PWD`.
    ///
    /// # Errors
    /// Returns `AgyError::Io` if querying current directory or canonicalization fails.
    pub fn find_root(start_dir: Option<&Path>) -> Result<PathBuf> {
        let current = if let Some(p) = start_dir {
            p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
        } else {
            let cwd = std::env::current_dir()?;
            Self::detect_plugin_sandbox_escape(&cwd).unwrap_or(cwd)
        };

        let mut candidate = current.as_path();
        loop {
            let jj_dir = candidate.join(".jj");
            if jj_dir.is_dir() {
                let repo_ptr = jj_dir.join("repo");
                if repo_ptr.is_file() {
                    // Secondary Jujutsu workspace: pointer file references main repo's .jj/repo
                    if let Ok(target) = std::fs::read_to_string(&repo_ptr) {
                        let trimmed = target.trim();
                        let full_target = jj_dir.join(trimmed);
                        if let Ok(canonical_target) = full_target.canonicalize()
                            && let Some(main_repo) =
                                canonical_target.parent().and_then(|p| p.parent())
                        {
                            return Ok(main_repo.to_path_buf());
                        }
                    }
                }
                return candidate.canonicalize().map_err(AgyError::Io);
            }

            let git_dir = candidate.join(".git");
            if git_dir.exists() {
                if git_dir.is_file() {
                    // Git worktree or submodule pointer
                    if let Ok(content) = std::fs::read_to_string(&git_dir)
                        && let Some(gitdir_rel) = content.trim().strip_prefix("gitdir:")
                    {
                        let full_gitdir = candidate.join(gitdir_rel.trim());
                        if let Ok(canonical_gitdir) = full_gitdir.canonicalize() {
                            if canonical_gitdir.to_string_lossy().contains("worktrees") {
                                if let Some(repo) = canonical_gitdir
                                    .parent()
                                    .and_then(|p| p.parent())
                                    .and_then(|p| p.parent())
                                {
                                    return Ok(repo.to_path_buf());
                                }
                            } else if let Some(main_repo) = canonical_gitdir.parent() {
                                return Ok(main_repo.to_path_buf());
                            }
                        }
                    }
                }
                return candidate.canonicalize().map_err(AgyError::Io);
            }

            match candidate.parent() {
                Some(parent) => candidate = parent,
                None => break,
            }
        }

        // Fallback to start_dir canonicalized
        current.canonicalize().map_err(AgyError::Io)
    }

    /// Detects if `cwd` is locked inside an Antigravity plugin installation directory
    /// and recovers the caller's actual project directory from `$PWD` if valid.
    #[must_use]
    pub fn detect_plugin_sandbox_escape(cwd: &Path) -> Option<PathBuf> {
        let canon_cwd = cwd.canonicalize().ok()?;
        let cwd_str = canon_cwd.to_string_lossy();

        let is_in_plugin_dir = std::env::var("PLUGIN_ROOT").map_or_else(
            |_| {
                cwd_str.contains("/.gemini/config/plugins/")
                    || cwd_str.contains("/.antigravity/plugins/")
                    || cwd_str.contains("/.gemini/plugins/")
            },
            |plugin_root| {
                Path::new(&plugin_root)
                    .canonicalize()
                    .is_ok_and(|canon_plugin| canon_cwd.starts_with(&canon_plugin))
            },
        );

        if is_in_plugin_dir && let Ok(pwd_val) = std::env::var("PWD") {
            let pwd_path = PathBuf::from(pwd_val);
            if let Ok(canon_pwd) = pwd_path.canonicalize()
                && canon_pwd.is_dir()
                && !canon_pwd.starts_with(&canon_cwd)
            {
                return Some(canon_pwd);
            }
        }

        None
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
    fn test_find_root_on_current_repo() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = WorkspaceResolver::find_root(Some(&manifest_dir)).unwrap();
        assert!(root.join("Cargo.toml").is_file());
    }

    #[test]
    fn test_find_root_from_secondary_jj_workspace_pointer() {
        let temp_dir = std::env::temp_dir().join(format!(
            "agy_test_resolver_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let main_repo = temp_dir.join("main_repo");
        let main_jj_repo = main_repo.join(".jj/repo");
        std::fs::create_dir_all(&main_jj_repo).unwrap();

        let sub_ws = main_repo.join(".agy/workspaces/agent_sub");
        let sub_jj = sub_ws.join(".jj");
        std::fs::create_dir_all(&sub_jj).unwrap();
        std::fs::write(sub_jj.join("repo"), "../../../../.jj/repo").unwrap();

        let resolved = WorkspaceResolver::find_root(Some(&sub_ws)).unwrap();
        assert_eq!(resolved, main_repo.canonicalize().unwrap());

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_detect_plugin_sandbox_escape_inactive_outside_plugins() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // When running in a normal project directory, escape detection returns None
        assert!(WorkspaceResolver::detect_plugin_sandbox_escape(&manifest_dir).is_none());
    }
}
