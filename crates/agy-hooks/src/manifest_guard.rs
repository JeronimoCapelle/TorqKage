//! Manifest mutation guard blocking unauthorized build manifest modifications.
//!
//! Neutralizes the host IDE procedural-macro / `build.rs` execution sandbox escape vector.

use std::path::Path;
use agy_common::{PreToolResponse, PreToolUseInput};
use crate::config::HooksConfig;

pub(crate) fn is_default_fallback_mutation_tool(name: &str) -> bool {
    let bytes = name.as_bytes();
    if matches!(bytes, b"file_write" | b"write_file" | b"mpatch_edit" | b"modify_code" | b"edit_file") {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    lower.contains("write")
        || lower.contains("edit")
        || lower.contains("modify")
        || lower.contains("patch")
        || lower.contains("create")
        || lower.contains("append")
        || lower.contains("delete")
        || lower.contains("remove")
        || lower.contains("overwrite")
        || lower.contains("replace")
}

fn is_path_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.contains("path") || k.contains("file") || k == "dest" || k == "destination"
}

fn is_candidate_path(s: &str) -> bool {
    let trimmed = s.trim();
    !trimmed.is_empty() && !trimmed.contains('\n') && trimmed.len() <= 4096
}

/// Extracts candidate target file path arguments from a tool invocation payload.
///
/// Priority 1: Inspect keys matching common path or file conventions (`path`, `Path`, `file`, `dest`, etc.).
/// Priority 2: If no semantic path keys are present, inspect any single-line string values in the arguments.
pub(crate) fn extract_candidate_paths(args: &serde_json::Value) -> Vec<&str> {
    let mut candidates = Vec::new();

    if let serde_json::Value::Object(map) = args {
        // Priority 1: Keys that look like path/file arguments
        for (k, v) in map {
            if is_path_key(k)
                && let Some(s) = v.as_str()
                && is_candidate_path(s)
            {
                candidates.push(s.trim());
            }
        }

        // Priority 2: If no semantic path keys found, inspect any string value that looks like a path (single-line)
        if candidates.is_empty() {
            for v in map.values() {
                if let Some(s) = v.as_str()
                    && is_candidate_path(s)
                {
                    candidates.push(s.trim());
                }
            }
        }
    }

    candidates
}

fn is_manifest_protected(path_str: &str, patterns: &[String]) -> bool {
    let p = Path::new(path_str);
    let file_name = p.file_name().map(|f| f.to_string_lossy());

    patterns.iter().any(|pat| {
        if pat.starts_with('*') {
            pat.strip_prefix('*').is_some_and(|suffix| {
                file_name.as_ref().is_some_and(|f| f.ends_with(suffix))
                    || path_str.ends_with(suffix)
            })
        } else {
            file_name.as_ref().is_some_and(|f| f.eq_ignore_ascii_case(pat))
                || path_str.eq_ignore_ascii_case(pat)
        }
    })
}

/// Evaluates whether a tool invocation attempts to modify a protected build manifest.
///
/// If a protected manifest (e.g. `Cargo.toml`, `build.rs`, `CMakeLists.txt`) is targeted,
/// returns a `PreToolResponse::ask` to require explicit operator confirmation before modification.
#[must_use]
pub(crate) fn evaluate_manifest_guard(
    input: &PreToolUseInput,
    root: &Path,
    config: &HooksConfig,
) -> Option<PreToolResponse> {
    if !config.manifest_guard.enabled {
        return None;
    }

    let tc = input.tool_call.as_ref()?;
    let tool_name = tc.name.as_str();

    let is_mutation_tool = if config.manifest_guard.protected_tools.is_empty() {
        crate::policy_gate::is_mutating_tool(tool_name, root)
            || is_default_fallback_mutation_tool(tool_name)
    } else {
        config.manifest_guard.protected_tools.iter().any(|t| t == tool_name)
    };
    if !is_mutation_tool {
        return None;
    }

    let candidates = extract_candidate_paths(&tc.args);
    for target_path in candidates {
        if is_manifest_protected(target_path, &config.manifest_guard.protected_patterns) {
            return Some(PreToolResponse::ask(format!(
                "🛑 MANIFEST MUTATION GUARD: Tool '{tool_name}' attempted to modify protected build manifest '{target_path}'.\n\
                 Modifying build manifests can trigger un-sandboxed host IDE proc-macro or build-script execution.\n\
                 Confirm manual override to allow modifying this manifest?"
            )));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use agy_common::ToolCallPayload;
    use serde_json::json;

    #[test]
    fn test_manifest_guard_blocks_cargo_toml() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "file_write".to_owned(),
                args: json!({
                    "path": "Cargo.toml",
                    "content": "[dependencies]\nevil-proc-macro = \"1.0\""
                }),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let resp = evaluate_manifest_guard(&input, Path::new("."), &config);
        assert!(resp.is_some());
        if let Some(res) = resp {
            assert_eq!(res.decision, agy_common::HookDecision::Ask);
            assert!(res.reason.as_deref().unwrap_or("").contains("MANIFEST MUTATION GUARD"));
        }
    }

    #[test]
    fn test_manifest_guard_blocks_build_rs() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "write_file".to_owned(),
                args: json!({
                    "Path": "crates/my-crate/build.rs",
                    "content": "fn main() {}"
                }),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let resp = evaluate_manifest_guard(&input, Path::new("."), &config);
        assert!(resp.is_some());
        if let Some(res) = resp {
            assert_eq!(res.decision, agy_common::HookDecision::Ask);
        }
    }

    #[test]
    fn test_manifest_guard_allows_src_files() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "file_write".to_owned(),
                args: json!({
                    "path": "src/lib.rs",
                    "content": "pub fn hello() {}"
                }),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let resp = evaluate_manifest_guard(&input, Path::new("."), &config);
        assert!(resp.is_none());
    }

    #[test]
    fn test_manifest_guard_custom_key_destination() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "file_write".to_owned(),
                args: json!({
                    "destination": "package.json",
                    "content": "{}"
                }),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let resp = evaluate_manifest_guard(&input, Path::new("."), &config);
        assert!(resp.is_some());
        if let Some(res) = resp {
            assert_eq!(res.decision, agy_common::HookDecision::Ask);
        }
    }

    #[test]
    fn test_manifest_guard_no_false_positive_on_multiline_content() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "file_write".to_owned(),
                args: json!({
                    "path": "src/parser.rs",
                    "content": "// Refer to Cargo.toml for dependencies\npub fn parse() {}"
                }),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let resp = evaluate_manifest_guard(&input, Path::new("."), &config);
        assert!(resp.is_none());
    }
}
