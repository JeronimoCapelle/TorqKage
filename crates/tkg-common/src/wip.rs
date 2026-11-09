//! Working-in-progress (WIP) working tree change detector and cache tracker.

use std::{
    hash::{Hash, Hasher},
    path::Path,
};

use crate::{JujutsuWorkspace, ProjectConfig, TkgStatePaths};

/// Result of evaluating working tree modifications across conversational turns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WipCheckResult {
    /// No uncommitted working copy changes detected.
    Clean,
    /// Working copy modified, but identical to the previous check.
    Unchanged { count: usize },
    /// New working copy modifications detected since previous check.
    Modified {
        files: Vec<String>,
        total_count: usize,
    },
}

impl WipCheckResult {
    /// Returns `true` if the working copy is clean with zero uncommitted changes.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        matches!(self, Self::Clean)
    }

    /// Formats an ephemeral notification if working copy changes occurred.
    ///
    /// Returns `None` if clean or unchanged to conserve model tokens.
    #[must_use]
    pub fn format_notice(&self) -> Option<String> {
        match self {
            Self::Clean | Self::Unchanged { .. } => None,
            Self::Modified { files, .. } => {
                if files.is_empty() {
                    Some("[Notice: Working copy modified on disk]".to_owned())
                } else {
                    Some(format!(
                        "[Notice: Files modified on disk: {}]",
                        files.join(", ")
                    ))
                }
            },
        }
    }

    /// Formats a human-readable status message for MCP or status commands.
    #[must_use]
    pub fn format_status_message(&self) -> String {
        match self {
            Self::Clean => "Working copy clean (0 uncommitted changes).".to_owned(),
            Self::Unchanged { count } => {
                format!("Working copy unchanged since previous turn ({count} modified file(s)).")
            },
            Self::Modified { files, .. } => {
                if files.is_empty() {
                    "[Notice: Working copy modified on disk]".to_owned()
                } else {
                    format!("[Notice: Files modified on disk: {}]", files.join(", "))
                }
            },
        }
    }
}

/// Evaluates uncommitted changes against `.tkg/cache/wip/` for a given session.
///
/// Computes an in-memory hash of `git status -s` or `jj diff --summary`.
/// If unchanged or clean, avoids generating redundant warnings.
/// If modified, records the new hash in the session cache and returns the list of modified files.
#[must_use]
pub fn check_wip_changes(
    workspace_root: &Path,
    conversation_id: Option<&str>,
    max_preview: usize,
) -> WipCheckResult {
    let project = ProjectConfig::detect_or_load(workspace_root);
    let status_str = JujutsuWorkspace::get_working_copy_diff_summary(workspace_root, &project);

    let trimmed = status_str.trim();
    if trimmed.is_empty() {
        return WipCheckResult::Clean;
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    trimmed.hash(&mut hasher);
    let current_hash = hasher.finish();

    let paths = TkgStatePaths::new(workspace_root);
    let cache_file = paths.wip_cache_file(conversation_id);

    if let Ok(cached) = std::fs::read_to_string(&cache_file)
        && cached.trim() == format!("{current_hash}")
    {
        let count = trimmed.lines().count();
        return WipCheckResult::Unchanged { count };
    }

    if let Some(parent) = cache_file.parent() {
        drop(std::fs::create_dir_all(parent));
    }
    drop(std::fs::write(&cache_file, format!("{current_hash}")));

    let total_count = trimmed.lines().count();
    let modified_files: Vec<String> = trimmed
        .lines()
        .take(max_preview)
        .map(|line| {
            let t = line.trim();
            if let Some((_, path)) = t.split_once(' ') {
                path.trim().to_owned()
            } else {
                t.to_owned()
            }
        })
        .collect();

    WipCheckResult::Modified {
        files: modified_files,
        total_count,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_check_wip_changes_clean() {
        let temp_dir = std::env::temp_dir().join(format!(
            "common_wip_clean_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&temp_dir));

        let res = check_wip_changes(&temp_dir, Some("test_session"), 10);
        assert!(res.is_clean());
        assert_eq!(res.format_notice(), None);
        assert!(res.format_status_message().contains("Working copy clean"));

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_wip_check_result_formatting() {
        let clean = WipCheckResult::Clean;
        assert_eq!(clean.format_notice(), None);
        assert!(
            clean
                .format_status_message()
                .contains("0 uncommitted changes")
        );

        let unchanged = WipCheckResult::Unchanged { count: 3 };
        assert_eq!(unchanged.format_notice(), None);
        assert!(
            unchanged
                .format_status_message()
                .contains("unchanged since previous turn (3 modified")
        );

        let modified = WipCheckResult::Modified {
            files: vec!["src/lib.rs".to_owned(), "Cargo.toml".to_owned()],
            total_count: 2,
        };
        assert_eq!(
            modified.format_notice(),
            Some("[Notice: Files modified on disk: src/lib.rs, Cargo.toml]".to_owned())
        );
        assert_eq!(
            modified.format_status_message(),
            "[Notice: Files modified on disk: src/lib.rs, Cargo.toml]"
        );
    }
}
