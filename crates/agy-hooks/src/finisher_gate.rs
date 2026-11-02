//! Pre-tool gate for `invoke_subagent` calls targeting `finisher` (`check-finisher`).

use std::path::PathBuf;
use agy_common::{PreToolResponse, PreToolUseInput, WorkspaceResolver};
use serde_json::Value;

use crate::stdin;

/// Executes the `check-finisher` command.
///
/// Inspects incoming `PreToolUseInput` on `stdin`. If the tool call is `invoke_subagent`
/// with target type `finisher`, it executes all modular gate checks (compilation, test compilation,
/// VCS merge conflicts). If any checks fail, it outputs a blocking `PreToolResponse::deny` payload.
/// Otherwise, outputs `PreToolResponse::allow`.
pub(crate) fn execute() {
    let input = stdin::read_stdin_payload::<PreToolUseInput>();
    let response = evaluate_tool_invocation(&input);
    if let Ok(json) = serde_json::to_string(&response) {
        println!("{json}");
    }
}

/// Evaluates a tool invocation input and produces a `PreToolResponse`.
pub(crate) fn evaluate_tool_invocation(input: &PreToolUseInput) -> PreToolResponse {
    let Some(ref tc) = input.tool_call else {
        return PreToolResponse::allow();
    };

    if tc.name != "invoke_subagent" {
        return PreToolResponse::allow();
    }

    let type_name = tc
        .args
        .get("TypeName")
        .or_else(|| tc.args.get("typeName"))
        .or_else(|| tc.args.get("type_name"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    if type_name.is_empty() {
        return PreToolResponse::allow();
    }

    // Resolve workspace root from payload or current working directory
    let root = input
        .common
        .workspace_paths
        .first()
        .cloned()
        .unwrap_or_else(|| WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from(".")));

    // 1. Check if the target agent declares pre_launch_checks in its markdown file
    let checks = if let Some(path) = agy_common::find_agent_manifest_path(&root, type_name)
        && let Ok(content) = std::fs::read_to_string(&path)
    {
        let declared = agy_common::parse_pre_launch_checks(&content);
        if declared.is_empty() && type_name.eq_ignore_ascii_case("finisher") {
            vec!["cargo_compilation".to_owned(), "test_compilation".to_owned(), "vcs_conflicts".to_owned()]
        } else {
            declared
        }
    } else if type_name.eq_ignore_ascii_case("finisher") {
        vec!["cargo_compilation".to_owned(), "test_compilation".to_owned(), "vcs_conflicts".to_owned()]
    } else {
        Vec::new()
    };

    if checks.is_empty() {
        return PreToolResponse::allow();
    }

    let check_refs: Vec<&str> = checks.iter().map(String::as_str).collect();
    match agy_common::evaluate_pre_flight_checks(&root, &check_refs) {
        Ok(_warnings) => PreToolResponse::allow(),
        Err(errors) => {
            let reason = format!(
                "🛑 Subagent Admission Denied for '{type_name}': Mandatory pre-launch quality gates failed.\n\
                 Target agent requirements: [{}]\n\n\
                 {}",
                checks.join(", "),
                errors.join("\n\n")
            );
            PreToolResponse::deny(reason)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agy_common::models::ToolCallPayload;
    use serde_json::json;

    #[test]
    fn test_non_subagent_tool_is_allowed() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "run_command".to_owned(),
                args: json!({"CommandLine": "cargo test"}),
            }),
            ..Default::default()
        };

        let res = evaluate_tool_invocation(&input);
        assert_eq!(res.decision, agy_common::models::HookDecision::Allow);
    }

    #[test]
    fn test_other_subagent_is_allowed() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "invoke_subagent".to_owned(),
                args: json!({
                    "TypeName": "mini",
                    "Role": "Tester",
                    "Prompt": "Calculate 2+2"
                }),
            }),
            ..Default::default()
        };

        let res = evaluate_tool_invocation(&input);
        assert_eq!(res.decision, agy_common::models::HookDecision::Allow);
    }

    #[test]
    fn test_finisher_subagent_evaluates_readiness() {
        // Point workspace to temp dir without Cargo.toml so checks succeed
        let temp_dir = std::env::temp_dir();
        let mut input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "invoke_subagent".to_owned(),
                args: json!({
                    "TypeName": "finisher",
                    "Role": "Release auditor",
                    "Prompt": "Check PR readiness"
                }),
            }),
            ..Default::default()
        };
        input.common.workspace_paths = vec![temp_dir];

        let res = evaluate_tool_invocation(&input);
        assert_eq!(res.decision, agy_common::models::HookDecision::Allow);
    }
}
