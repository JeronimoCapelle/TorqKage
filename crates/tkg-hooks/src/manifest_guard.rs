//! Manifest mutation guard blocking unauthorized build manifest modifications.
//!
//! Neutralizes the host IDE procedural-macro / `build.rs` execution sandbox escape vector.

use std::path::Path;

pub(crate) use tkg_common::manifest_guard::{
    extract_candidate_paths, is_default_fallback_mutation_tool, is_manifest_protected,
};
use tkg_common::{PreToolResponse, PreToolUseInput};

use crate::config::HooksConfig;

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
        config
            .manifest_guard
            .protected_tools
            .iter()
            .any(|t| t == tool_name)
    };
    if !is_mutation_tool {
        return None;
    }

    let candidates = extract_candidate_paths(&tc.args);
    for target_path in &candidates {
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
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use serde_json::json;
    use tkg_common::{HookDecision, ToolCallPayload};

    use super::*;

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
        let res = resp.unwrap();
        assert_eq!(res.decision, HookDecision::Ask);
        assert!(res.reason.unwrap().contains("MANIFEST MUTATION GUARD"));
    }

    #[test]
    fn test_manifest_guard_allows_regular_source() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "file_write".to_owned(),
                args: json!({
                    "path": "src/main.rs",
                    "content": "fn main() {}"
                }),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let resp = evaluate_manifest_guard(&input, Path::new("."), &config);
        assert!(resp.is_none());
    }

    #[test]
    fn test_manifest_guard_allows_read_only_tool() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "file_read".to_owned(),
                args: json!({
                    "path": "Cargo.toml"
                }),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let resp = evaluate_manifest_guard(&input, Path::new("."), &config);
        assert!(resp.is_none());
    }

    #[test]
    fn test_manifest_guard_disabled() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "file_write".to_owned(),
                args: json!({
                    "path": "Cargo.toml"
                }),
            }),
            ..Default::default()
        };

        let mut config = HooksConfig::default();
        config.manifest_guard.enabled = false;
        let resp = evaluate_manifest_guard(&input, Path::new("."), &config);
        assert!(resp.is_none());
    }
}
