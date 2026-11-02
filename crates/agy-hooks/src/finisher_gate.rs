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
    // Determine target agent type from tool call arguments
    let is_finisher = input.tool_call.as_ref().is_some_and(|tc| {
        if tc.name != "invoke_subagent" {
            return false;
        }

        let type_name = tc
            .args
            .get("TypeName")
            .or_else(|| tc.args.get("typeName"))
            .or_else(|| tc.args.get("type_name"))
            .and_then(Value::as_str)
            .unwrap_or_default();

        type_name.eq_ignore_ascii_case("finisher")
    });

    // If this invocation is not dispatching the finisher agent, immediately allow
    if !is_finisher {
        return PreToolResponse::allow();
    }

    // Resolve workspace root from payload or current working directory
    let root = input
        .common
        .workspace_paths
        .first()
        .cloned()
        .unwrap_or_else(|| WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from(".")));

    // Run modular finisher quality gate checks
    match agy_common::evaluate_finisher_readiness(&root) {
        Ok(_warnings) => PreToolResponse::allow(),
        Err(errors) => {
            let reason = format!(
                "🛑 Finisher Gatekeeper Denied: Project does not compile or has unresolved conflicts.\n\
                 The finisher agent cannot be invoked until the codebase builds cleanly and all tests compile.\n\n\
                 {}",
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
