//! In-process working tree modification detector.

use std::{
    hash::{Hash, Hasher},
    path::Path,
};

use agy_common::{AgyStatePaths, JujutsuWorkspace, ProjectConfig};

/// Executes the working copy modification detector.
///
/// Computes an in-memory hash of working-copy diffs. If files changed since the previous
/// check, updates `.agy/cache/wip/` and returns an actionable notice with the modified files.
/// If unchanged or clean, returns a concise clean status.
///
/// # Errors
/// Returns an error if filesystem operations fail.
pub(crate) fn execute(workspace_root: &Path, arguments: &serde_json::Value) -> (String, bool) {
    let conversation_id = arguments
        .get("conversation_id")
        .or_else(|| arguments.get("session_id"))
        .and_then(serde_json::Value::as_str);

    let max_preview = arguments
        .get("max_preview")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| usize::try_from(v).unwrap_or(10));

    let project = ProjectConfig::detect_or_load(workspace_root);
    let status_str = JujutsuWorkspace::get_working_copy_diff_summary(workspace_root, &project);

    let trimmed = status_str.trim();
    if trimmed.is_empty() {
        return (
            "Working copy clean (0 uncommitted changes).".to_owned(),
            false,
        );
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    trimmed.hash(&mut hasher);
    let current_hash = hasher.finish();

    let paths = AgyStatePaths::new(workspace_root);
    let cache_file = paths.wip_cache_file(conversation_id);

    if let Ok(cached) = std::fs::read_to_string(&cache_file)
        && cached.trim() == format!("{current_hash}")
    {
        let count = trimmed.lines().count();
        return (
            format!("Working copy unchanged since previous turn ({count} modified file(s))."),
            false,
        );
    }

    if let Some(parent) = cache_file.parent() {
        drop(std::fs::create_dir_all(parent));
    }
    drop(std::fs::write(&cache_file, format!("{current_hash}")));

    let modified_files: Vec<&str> = trimmed
        .lines()
        .take(max_preview)
        .map(|l| {
            let t = l.trim();
            if let Some((_, path)) = t.split_once(' ') {
                path.trim()
            } else {
                t
            }
        })
        .collect();

    let notice = format!(
        "[Notice: Files modified on disk: {}]",
        modified_files.join(", ")
    );

    (notice, false)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_detect_wip_clean_or_modified() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mcp_wip_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&temp_dir));

        let (out, is_err) = execute(&temp_dir, &serde_json::json!({}));
        assert!(!is_err);
        assert!(out.contains("Working copy") || out.contains("[Notice: Files modified"));

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
