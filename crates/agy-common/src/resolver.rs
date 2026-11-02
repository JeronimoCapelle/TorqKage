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
    /// (e.g. `.workspaces/<agent>`) by resolving pointer files in `.jj/repo`.
    ///
    /// # Errors
    /// Returns `AgyError::Io` if querying current directory or canonicalization fails.
    pub fn find_root(start_dir: Option<&Path>) -> Result<PathBuf> {
        let current = match start_dir {
            Some(p) => p.to_path_buf(),
            None => std::env::current_dir()?,
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
                            && let Some(main_repo) = canonical_target.parent().and_then(|p| p.parent())
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
                                if let Some(repo) = canonical_gitdir.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
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
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let main_repo = temp_dir.join("main_repo");
        let main_jj_repo = main_repo.join(".jj/repo");
        std::fs::create_dir_all(&main_jj_repo).unwrap();

        let sub_ws = main_repo.join(".workspaces/coder");
        let sub_jj = sub_ws.join(".jj");
        std::fs::create_dir_all(&sub_jj).unwrap();
        std::fs::write(sub_jj.join("repo"), "../../../.jj/repo").unwrap();

        let resolved = WorkspaceResolver::find_root(Some(&sub_ws)).unwrap();
        assert_eq!(resolved, main_repo.canonicalize().unwrap());

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
