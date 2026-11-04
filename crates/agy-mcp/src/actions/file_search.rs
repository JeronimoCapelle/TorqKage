//! `file_search` action handler providing fast pattern search across repository files.

use std::path::Path;
use agy_common::{ProjectConfig, ProjectSecurityConfig};
use crate::guards::{check_dangerous_files_with_security, truncate_output};
use crate::models::ToolDefinition;

/// Executes a bounded pattern search across files.
///
/// If no explicit path is given, searches across all configured documentation paths
/// in `project.toml` (e.g. `docs/`, `specs/`, `ARCHITECTURE.md`), or falls back to `tool.enforce_dir`.
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

    let project = ProjectConfig::detect_or_load(workspace_root);
    let pattern_lower = pattern.to_lowercase();
    let mut matches = Vec::new();

    let path_arg = arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let (targets, is_docs_query) = resolve_search_targets(tool, workspace_root, &project, path_arg);

    let mut searched_targets = Vec::new();
    for target in &targets {
        if !target.exists() {
            continue;
        }

        let rel_display = target
            .strip_prefix(workspace_root)
            .unwrap_or(target)
            .display()
            .to_string();
        searched_targets.push(rel_display);

        if target.is_dir() {
            walk_and_search(target, workspace_root, &pattern_lower, &project.security, &mut matches)?;
        } else if target.is_file() {
            search_single_file(target, workspace_root, &pattern_lower, &project.security, &mut matches);
        }
    }

    if searched_targets.is_empty() {
        if is_docs_query {
            return Ok(format!(
                "No documentation or specification paths found on disk (configured: [{}]). Configure valid paths in project.toml under [docs] paths = [...].",
                project.docs.display_summary()
            ));
        }
        let fallback_display = targets
            .first()
            .map_or_else(|| ".".to_owned(), |p| p.strip_prefix(workspace_root).unwrap_or(p).display().to_string());
        return Ok(format!(
            "Search target directory '{fallback_display}' does not exist in repository."
        ));
    }

    if matches.is_empty() {
        let searched_summary = searched_targets.join(", ");
        return Ok(format!(
            "No occurrences found for pattern '{pattern}' in: {searched_summary}."
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

fn resolve_search_targets(
    tool: &ToolDefinition,
    workspace_root: &Path,
    project: &ProjectConfig,
    path_arg: Option<&str>,
) -> (Vec<std::path::PathBuf>, bool) {
    if let Some(custom) = path_arg {
        return (vec![workspace_root.join(custom)], false);
    }

    if !project.docs.paths.is_empty()
        && (tool.name == "query_project_spec" || tool.enforce_dir.as_deref() == Some("docs"))
    {
        let targets = project
            .docs
            .paths
            .iter()
            .map(|p| workspace_root.join(p))
            .collect();
        return (targets, true);
    }

    if let Some(ref sub) = tool.enforce_dir {
        return (vec![workspace_root.join(sub)], false);
    }

    (vec![workspace_root.to_path_buf()], false)
}

fn search_single_file(
    file_path: &Path,
    workspace_root: &Path,
    pattern_lower: &str,
    security: &ProjectSecurityConfig,
    matches: &mut Vec<String>,
) {
    if check_dangerous_files_with_security(file_path, Some(security)).is_err() {
        return;
    }

    if let Ok(content) = std::fs::read_to_string(file_path) {
        let rel_path = file_path
            .strip_prefix(workspace_root)
            .unwrap_or(file_path)
            .display()
            .to_string();

        for (idx, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(pattern_lower) {
                let line_num = idx.saturating_add(1);
                matches.push(format!("{rel_path}:{line_num}: {line}"));
                if matches.len() >= 500 {
                    return;
                }
            }
        }
    }
}

fn walk_and_search(
    dir: &Path,
    workspace_root: &Path,
    pattern_lower: &str,
    security: &ProjectSecurityConfig,
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
            walk_and_search(&path, workspace_root, pattern_lower, security, matches)?;
        } else if path.is_file() {
            search_single_file(&path, workspace_root, pattern_lower, security, matches);
            if matches.len() >= 500 {
                return Ok(());
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

    #[test]
    fn test_file_search_across_multiple_configured_doc_paths() {
        let temp = create_test_dir("multi-docs");
        std::fs::create_dir_all(temp.join("docs")).unwrap();
        std::fs::write(
            temp.join("project.toml"),
            "[project]\nname = \"demo\"\n\n[docs]\npaths = [\"docs\", \"ARCHITECTURE.md\"]\n",
        )
        .unwrap();
        std::fs::write(
            temp.join("docs/specs.md"),
            "# Specs\nCACHE_LINE_SIZE = 64;\n",
        )
        .unwrap();
        std::fs::write(
            temp.join("ARCHITECTURE.md"),
            "# Architecture\nCACHE_LINE_SIZE is tuned for x86_64 and aarch64.\n",
        )
        .unwrap();

        let tool = ToolDefinition {
            name: "query_project_spec".to_owned(),
            ..Default::default()
        };

        // 1. Search without explicit path -> searches all configured doc paths
        let args_all = json!({ "pattern": "CACHE_LINE_SIZE" });
        let result_all = execute(&tool, &temp, &args_all).expect("search all docs");
        assert!(result_all.contains("docs/specs.md:2: CACHE_LINE_SIZE = 64;"));
        assert!(result_all.contains("ARCHITECTURE.md:2: CACHE_LINE_SIZE is tuned"));

        // 2. Search specific subpath "docs"
        let args_docs = json!({ "pattern": "CACHE_LINE_SIZE", "path": "docs" });
        let result_docs = execute(&tool, &temp, &args_docs).expect("search docs only");
        assert!(result_docs.contains("docs/specs.md:2: CACHE_LINE_SIZE = 64;"));
        assert!(!result_docs.contains("ARCHITECTURE.md"));

        // 3. Search specific file "ARCHITECTURE.md"
        let args_arch = json!({ "pattern": "CACHE_LINE_SIZE", "path": "ARCHITECTURE.md" });
        let result_arch = execute(&tool, &temp, &args_arch).expect("search architecture only");
        assert!(result_arch.contains("ARCHITECTURE.md:2: CACHE_LINE_SIZE is tuned"));
        assert!(!result_arch.contains("docs/specs.md"));

        drop(std::fs::remove_dir_all(&temp));
    }
}
