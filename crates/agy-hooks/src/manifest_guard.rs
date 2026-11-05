//! Manifest mutation guard blocking unauthorized build manifest modifications.
//!
//! Neutralizes the host IDE procedural-macro / `build.rs` execution sandbox escape vector.

use std::path::Path;
use agy_common::{PreToolResponse, PreToolUseInput};
use crate::config::HooksConfig;

const fn is_default_fallback_mutation_tool(name: &str) -> bool {
    let bytes = name.as_bytes();
    matches!(bytes, b"file_write" | b"write_file" | b"mpatch_edit" | b"modify_code" | b"edit_file")
}

/// Extracts the target file path argument from a tool invocation payload.
fn extract_target_path<'a>(args: &'a serde_json::Value, keys: &[String]) -> Option<&'a str> {
    for key in keys {
        if let Some(val) = args.get(key).and_then(serde_json::Value::as_str) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }

    None
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

    let target_path = extract_target_path(&tc.args, &config.manifest_guard.target_keys)?;
    let p = Path::new(target_path);
    let file_name = p.file_name()?.to_string_lossy();

    let is_protected = config
        .manifest_guard
        .protected_patterns
        .iter()
        .any(|pat| {
            if pat.starts_with('*') {
                pat.strip_prefix('*').is_some_and(|suffix| file_name.ends_with(suffix))
            } else {
                file_name.eq_ignore_ascii_case(pat) || target_path.eq_ignore_ascii_case(pat)
            }
        });

    if is_protected {
        return Some(PreToolResponse::ask(format!(
            "🛑 MANIFEST MUTATION GUARD: Tool '{tool_name}' attempted to modify protected build manifest '{target_path}'.\n\
             Modifying build manifests can trigger un-sandboxed host IDE proc-macro or build-script execution.\n\
             Confirm manual override to allow modifying this manifest?"
        )));
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
}
