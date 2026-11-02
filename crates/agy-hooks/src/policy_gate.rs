//! Granular tool-level permission policy enforcement (`enforce-tool-policy`).

use std::path::{Path, PathBuf};
use agy_common::{PreToolResponse, PreToolUseInput, WorkspaceResolver};

use crate::stdin;

/// Executes the `enforce-tool-policy` command.
///
/// Inspects incoming `PreToolUseInput` on `stdin`. Evaluates whether the requested tool
/// requires human confirmation (`ask`) or is authorized for autonomous execution (`allow`).
pub(crate) fn execute() {
    let input = stdin::read_stdin_payload::<PreToolUseInput>();
    let root = input
        .common
        .workspace_paths
        .first()
        .cloned()
        .unwrap_or_else(|| WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from(".")));

    let response = evaluate_policy(&input, &root);
    if let Ok(json) = serde_json::to_string(&response) {
        println!("{json}");
    }
}

/// Evaluates a tool call against the granular security policy.
pub(crate) fn evaluate_policy(input: &PreToolUseInput, root: &Path) -> PreToolResponse {
    let Some(ref tc) = input.tool_call else {
        return PreToolResponse::allow();
    };

    let tool_name = tc.name.as_str();

    // 1. Check if tool is explicitly declared as requiring permission in tools.toml
    if is_tool_gated_in_config(tool_name, root) {
        return PreToolResponse::ask(format!(
            "Policy Gate: Tool '{tool_name}' requires interactive operator confirmation before execution."
        ));
    }

    // 2. Default mutating tools that alter source code or repository manifests
    if is_default_mutating_tool(tool_name) {
        return PreToolResponse::ask(format!(
            "Policy Gate: Tool '{tool_name}' applies automated filesystem or code modifications. Confirm execution?"
        ));
    }

    // 3. All other verification and inspection tools are allowed autonomously
    PreToolResponse::allow()
}

/// Checks whether a tool is a default mutating tool that requires confirmation.
fn is_default_mutating_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "run_cargo_clippy_fix"
            | "modify_project_code"
            | "modify_source_code"
            | "modify_test_code"
            | "write_project_file"
            | "write_source_file"
            | "write_test_file"
            | "modify_design_doc"
            | "write_design_doc"
    )
}

/// Inspects tools.toml (if present) for `require_permission_gate = true`.
fn is_tool_gated_in_config(tool_name: &str, root: &Path) -> bool {
    let config_path = root.join("tools.toml");
    let plugin_config = root.join("plugins/agy-suite/tools.toml");

    let path_to_read = if config_path.is_file() {
        config_path
    } else if plugin_config.is_file() {
        plugin_config
    } else {
        return false;
    };

    if let Ok(content) = std::fs::read_to_string(path_to_read) {
        let header = format!("[tools.{tool_name}]");
        let mut in_section = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == header {
                in_section = true;
                continue;
            }
            if in_section {
                if trimmed.starts_with('[') {
                    break;
                }
                if trimmed.contains("require_permission_gate = true") {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;
    use agy_common::models::{HookDecision, ToolCallPayload};
    use serde_json::json;

    #[test]
    fn test_read_only_tool_is_allowed() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "read_project_file".to_owned(),
                args: json!({"path": "src/lib.rs"}),
            }),
            ..Default::default()
        };

        let res = evaluate_policy(&input, Path::new("."));
        assert_eq!(res.decision, HookDecision::Allow);
    }

    #[test]
    fn test_mutating_tool_triggers_ask() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "modify_project_code".to_owned(),
                args: json!({"path": "src/lib.rs", "diff": "..."}),
            }),
            ..Default::default()
        };

        let res = evaluate_policy(&input, Path::new("."));
        assert_eq!(res.decision, HookDecision::Ask);
        assert!(res.reason.unwrap().contains("Confirm execution"));
    }

    #[test]
    fn test_clippy_fix_triggers_ask() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "run_cargo_clippy_fix".to_owned(),
                args: json!({}),
            }),
            ..Default::default()
        };

        let res = evaluate_policy(&input, Path::new("."));
        assert_eq!(res.decision, HookDecision::Ask);
    }
}
