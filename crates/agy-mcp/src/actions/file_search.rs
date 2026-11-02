//! `file_search` action handler providing fast pattern search across repository files.

use std::path::Path;
use crate::guards::{check_dangerous_files, truncate_output};
use crate::models::ToolDefinition;

/// Executes a bounded pattern search across files.
///
/// # Errors
/// Returns an error message if parameters are invalid or search fails.
pub(crate) fn execute(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let pattern = arguments
        .get("pattern")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .ok_or_else(|| "Missing required parameter 'pattern'".to_owned())?;

    if pattern.is_empty() {
        return Err("Search pattern cannot be empty".to_owned());
    }

    let search_dir = tool
        .enforce_dir
        .as_ref()
        .map_or_else(|| workspace_root.to_path_buf(), |sub| workspace_root.join(sub));

    if !search_dir.exists() {
        return Ok(format!(
            "Search target directory '{}' does not exist in repository.",
            search_dir.strip_prefix(workspace_root).unwrap_or(&search_dir).display()
        ));
    }

    let mut matches = Vec::new();
    let pattern_lower = pattern.to_lowercase();

    walk_and_search(&search_dir, workspace_root, &pattern_lower, &mut matches)?;

    if matches.is_empty() {
        let display_dir = search_dir.strip_prefix(workspace_root).unwrap_or(&search_dir);
        return Ok(format!(
            "No occurrences found for pattern '{pattern}' in '{}'.",
            display_dir.display()
        ));
    }

    let joined = matches.join("\n");
    let truncated = truncate_output(
        &joined,
        tool.truncate_lines.or(Some(150)),
        tool.truncate_bytes,
        tool.emit_truncation_notice(),
    );

    Ok(truncated)
}

fn walk_and_search(
    dir: &Path,
    workspace_root: &Path,
    pattern_lower: &str,
    matches: &mut Vec<String>,
) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name_lossy = file_name.to_string_lossy();

        // Skip VCS, build artifacts, and hidden directories
        if name_lossy.starts_with('.') || name_lossy == "target" || name_lossy == "node_modules" {
            continue;
        }

        if path.is_dir() {
            walk_and_search(&path, workspace_root, pattern_lower, matches)?;
        } else if path.is_file() {
            // Guard: Never search inside sensitive files
            if check_dangerous_files(&path).is_err() {
                continue;
            }

            // Inspect file if text
            if let Ok(content) = std::fs::read_to_string(&path) {
                let rel_path = path
                    .strip_prefix(workspace_root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();

                for (idx, line) in content.lines().enumerate() {
                    if line.to_lowercase().contains(pattern_lower) {
                        let line_num = idx.saturating_add(1);
                        matches.push(format!("{rel_path}:{line_num}: {line}"));
                        if matches.len() >= 500 {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assertions_on_result_states,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "agy-test-search-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn test_file_search_finds_pattern() {
        let temp = create_test_dir("found");
        let docs_dir = temp.join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(
            docs_dir.join("spec.md"),
            "# Specification\n\nOPCODE_CALL = 0x10;\nOPCODE_RET = 0x20;\n",
        )
        .unwrap();

        let tool = ToolDefinition {
            name: "query_project_spec".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };

        let args = json!({
            "pattern": "OPCODE_CALL"
        });

        let result = execute(&tool, &temp, &args).unwrap();
        assert!(result.contains("docs/spec.md:3: OPCODE_CALL = 0x10;"));
    }

    #[test]
    fn test_file_search_empty_when_no_match() {
        let temp = create_test_dir("empty");
        let docs_dir = temp.join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("spec.md"), "Hello world\n").unwrap();

        let tool = ToolDefinition {
            name: "query_project_spec".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };

        let args = json!({
            "pattern": "NON_EXISTENT_PATTERN"
        });

        let result = execute(&tool, &temp, &args).unwrap();
        assert!(result.contains("No occurrences found"));
    }
}
