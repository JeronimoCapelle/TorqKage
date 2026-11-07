//! `file_search` action handler providing fast pattern and regex search across repository files.

use std::path::Path;

use agy_common::{ProjectConfig, ProjectSecurityConfig};
use regex::{Regex, RegexBuilder};

use crate::{
    guards::{check_dangerous_files_with_security, truncate_output},
    models::ToolDefinition,
};

const MAX_SEARCH_MATCHES: usize = 500;

#[derive(Debug)]
enum SearchMatcher {
    Regex(Regex),
    Literal {
        pattern: String,
        case_sensitive: bool,
    },
}

impl SearchMatcher {
    fn literal(pattern: &str, case_sensitive: bool) -> Self {
        let stored_pattern = if case_sensitive {
            pattern.to_owned()
        } else {
            pattern.to_lowercase()
        };
        Self::Literal {
            pattern: stored_pattern,
            case_sensitive,
        }
    }

    fn from_args(
        pattern: &str,
        is_regex_opt: Option<bool>,
        case_sensitive: bool,
    ) -> Result<Self, String> {
        match is_regex_opt {
            Some(true) => {
                let re = RegexBuilder::new(pattern)
                    .case_insensitive(!case_sensitive)
                    .multi_line(true)
                    .build()
                    .map_err(|err| format!("Invalid regex pattern '{pattern}': {err}"))?;
                Ok(Self::Regex(re))
            },
            Some(false) => Ok(Self::literal(pattern, case_sensitive)),
            None => {
                // Auto-compile as regex if valid; gracefully fallback to literal if regex syntax fails.
                RegexBuilder::new(pattern)
                    .case_insensitive(!case_sensitive)
                    .multi_line(true)
                    .build()
                    .map_or_else(
                        |_| Ok(Self::literal(pattern, case_sensitive)),
                        |re| Ok(Self::Regex(re)),
                    )
            },
        }
    }

    fn is_match(&self, line: &str) -> bool {
        match self {
            Self::Regex(re) => re.is_match(line),
            Self::Literal {
                pattern,
                case_sensitive,
            } => {
                if *case_sensitive {
                    line.contains(pattern)
                } else {
                    line.to_lowercase().contains(pattern)
                }
            },
        }
    }
}

/// Executes a bounded pattern search across files.
///
/// If no explicit path is given, searches across all configured documentation paths
/// in `project.toml` (e.g. `docs/`, `specs/`, `ARCHITECTURE.md`), or falls back to `tool.enforce_dir`.
///
/// Supports regular expressions via the `regex` crate and optional `is_regex` / `case_sensitive` parameters.
///
/// # Errors
/// Returns an error message if parameters are invalid, regex compilation fails, or search fails.
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

    let is_regex_opt = arguments
        .get("is_regex")
        .or_else(|| arguments.get("regex"))
        .and_then(serde_json::Value::as_bool);

    let case_sensitive = arguments
        .get("case_sensitive")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let matcher = SearchMatcher::from_args(pattern, is_regex_opt, case_sensitive)?;

    let project = ProjectConfig::detect_or_load(workspace_root);
    let mut collected_matches = Vec::new();

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
            walk_and_search(
                target,
                workspace_root,
                &matcher,
                &project.security,
                &mut collected_matches,
            );
        } else if target.is_file() {
            search_single_file(
                target,
                workspace_root,
                &matcher,
                &project.security,
                &mut collected_matches,
            );
        }
    }

    if searched_targets.is_empty() {
        if is_docs_query {
            return Ok(format!(
                "No documentation or specification paths found on disk (configured: [{}]). Configure valid paths in project.toml under [docs] paths = [...].",
                project.docs.display_summary()
            ));
        }
        let fallback_display = targets.first().map_or_else(
            || ".".to_owned(),
            |p| {
                p.strip_prefix(workspace_root)
                    .unwrap_or(p)
                    .display()
                    .to_string()
            },
        );
        return Ok(format!(
            "Search target directory '{fallback_display}' does not exist in repository."
        ));
    }

    if collected_matches.is_empty() {
        let searched_summary = searched_targets.join(", ");
        return Ok(format!(
            "No occurrences found for pattern '{pattern}' in: {searched_summary}."
        ));
    }

    let joined = collected_matches.join("\n");
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
    matcher: &SearchMatcher,
    security: &ProjectSecurityConfig,
    collected_matches: &mut Vec<String>,
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
            if matcher.is_match(line) {
                let line_num = idx.saturating_add(1);
                collected_matches.push(format!("{rel_path}:{line_num}: {line}"));
                if collected_matches.len() >= MAX_SEARCH_MATCHES {
                    return;
                }
            }
        }
    }
}

fn walk_and_search(
    dir: &Path,
    workspace_root: &Path,
    matcher: &SearchMatcher,
    security: &ProjectSecurityConfig,
    collected_matches: &mut Vec<String>,
) {
    let mut builder = ignore::WalkBuilder::new(dir);
    builder
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .require_git(false);

    for result in builder.build() {
        let Ok(entry) = result else { continue };

        let path = entry.path();
        if path.is_file() {
            let name_lossy = entry.file_name().to_string_lossy();
            if name_lossy == "target" || name_lossy == "node_modules" {
                continue;
            }
            search_single_file(path, workspace_root, matcher, security, collected_matches);
            if collected_matches.len() >= MAX_SEARCH_MATCHES {
                break;
            }
        }
    }
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
    use serde_json::json;

    use super::*;

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

    #[test]
    fn test_file_search_regex_matches() {
        let temp = create_test_dir("regex-matches");
        let docs_dir = temp.join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(
            docs_dir.join("spec.md"),
            "# Specification\n\nOPCODE_CALL = 0x10;\nOPCODE_RET = 0x20;\nDATA_STORE = 0x30;\n",
        )
        .unwrap();

        let tool = ToolDefinition {
            name: "query_project_spec".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };

        // Regex matching both OPCODE_CALL and OPCODE_RET
        let args = json!({
            "pattern": r"OPCODE_[A-Z]+\s*=\s*0x[0-9a-fA-F]+",
            "is_regex": true
        });

        let result = execute(&tool, &temp, &args).expect("execute regex search");
        assert!(result.contains("docs/spec.md:3: OPCODE_CALL = 0x10;"));
        assert!(result.contains("docs/spec.md:4: OPCODE_RET = 0x20;"));
        assert!(!result.contains("DATA_STORE"));

        drop(std::fs::remove_dir_all(&temp));
    }

    #[test]
    fn test_file_search_regex_auto_compile_without_flag() {
        let temp = create_test_dir("regex-auto");
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

        // Valid regex passed without is_regex flag -> should auto-compile as regex
        let args = json!({
            "pattern": "OPCODE_[A-Z]+"
        });

        let result = execute(&tool, &temp, &args).expect("auto regex search");
        assert!(result.contains("docs/spec.md:3: OPCODE_CALL = 0x10;"));
        assert!(result.contains("docs/spec.md:4: OPCODE_RET = 0x20;"));

        drop(std::fs::remove_dir_all(&temp));
    }

    #[test]
    fn test_file_search_regex_invalid_syntax_error() {
        let temp = create_test_dir("regex-invalid");
        let tool = ToolDefinition {
            name: "query_project_spec".to_owned(),
            ..Default::default()
        };

        let args = json!({
            "pattern": "OPCODE_([a-z+",
            "is_regex": true
        });

        let err = execute(&tool, &temp, &args).unwrap_err();
        assert!(err.contains("Invalid regex pattern"));

        drop(std::fs::remove_dir_all(&temp));
    }

    #[test]
    fn test_file_search_regex_fallback_on_invalid_syntax() {
        let temp = create_test_dir("regex-fallback");
        let docs_dir = temp.join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(
            docs_dir.join("notes.txt"),
            "Special line: unclosed(paren in text\n",
        )
        .unwrap();

        let tool = ToolDefinition {
            name: "query_project_spec".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };

        // Pattern has invalid regex syntax "unclosed(" but is_regex is omitted -> fallback to literal
        let args = json!({
            "pattern": "unclosed("
        });

        let result = execute(&tool, &temp, &args).expect("fallback to literal search");
        assert!(result.contains("Special line: unclosed(paren in text"));

        drop(std::fs::remove_dir_all(&temp));
    }

    #[test]
    fn test_file_search_case_sensitive() {
        let temp = create_test_dir("case-sensitive");
        let docs_dir = temp.join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("spec.md"), "lowercase = 1;\nUPPERCASE = 2;\n").unwrap();

        let tool = ToolDefinition {
            name: "query_project_spec".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };

        // Case-sensitive search for UPPERCASE
        let args_upper = json!({
            "pattern": "UPPERCASE",
            "case_sensitive": true
        });
        let res_upper = execute(&tool, &temp, &args_upper).expect("case sensitive upper");
        assert!(res_upper.contains("UPPERCASE = 2;"));

        // Case-sensitive search for lowercase (should match lowercase but not UPPERCASE)
        let args_lower = json!({
            "pattern": "lowercase",
            "case_sensitive": true
        });
        let res_lower = execute(&tool, &temp, &args_lower).expect("case sensitive lower");
        assert!(res_lower.contains("lowercase = 1;"));
        assert!(!res_lower.contains("UPPERCASE = 2;"));

        drop(std::fs::remove_dir_all(&temp));
    }

    #[test]
    fn test_file_search_literal_mode() {
        let temp = create_test_dir("literal-mode");
        let docs_dir = temp.join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(
            docs_dir.join("spec.md"),
            "Line with literal: OPCODE_[A-Z]+\nOther line: OPCODE_CALL\n",
        )
        .unwrap();

        let tool = ToolDefinition {
            name: "query_project_spec".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };

        // Literal search for "OPCODE_[A-Z]+" with is_regex = false
        let args = json!({
            "pattern": "OPCODE_[A-Z]+",
            "is_regex": false
        });
        let result = execute(&tool, &temp, &args).expect("literal mode");
        assert!(result.contains("Line with literal: OPCODE_[A-Z]+"));
        assert!(!result.contains("Other line: OPCODE_CALL"));

        drop(std::fs::remove_dir_all(&temp));
    }
}
