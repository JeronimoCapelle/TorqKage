//! Dynamic repository and workspace root resolution.

use std::path::{Path, PathBuf};
use crate::error::{AgyError, Result};

/// Resolves the canonical repository or workspace root.
#[derive(Debug, Clone)]
pub struct WorkspaceResolver;

impl WorkspaceResolver {
    /// Discovers the workspace root by walking up from `start_dir` looking for `.jj` or `.git`.
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
            if candidate.join(".jj").exists() || candidate.join(".git").exists() {
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
