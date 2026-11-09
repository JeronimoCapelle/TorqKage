//! Pre-tool gate for `invoke_subagent` calls validating declared `pre_launch_checks`.

use std::path::PathBuf;

use agy_common::{PreToolResponse, PreToolUseInput, WorkspaceResolver};
use serde_json::Value;

use crate::stdin;

/// Executes the `check-subagent` command.
///
/// Inspects incoming `PreToolUseInput` on `stdin`. If the tool call is `invoke_subagent`,
/// it parses the target agent's declared `pre_launch_checks` and executes all specified gates.
/// If any check fails, it outputs a blocking `PreToolResponse::deny` payload.
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
        .unwrap_or_else(|| {
            WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."))
        });

    // Evaluate declared pre_launch_checks via canonical subagent admission gate
    match agy_common::subagent_gate::check_subagent_admission(&root, type_name) {
        Ok(_) => PreToolResponse::allow(),
        Err(reason) => PreToolResponse::deny(reason),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions and timestamps")]
mod tests {
    use agy_common::models::ToolCallPayload;
    use serde_json::json;

    use super::*;

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
                    "TypeName": "helper_agent",
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
    fn test_subagent_evaluates_readiness() {
        let temp_dir = std::env::temp_dir().join(format!(
            "admission_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let agents_dir = temp_dir.join("plugins/agy-suite/agents");
        drop(std::fs::create_dir_all(&agents_dir));
        drop(std::fs::write(
            agents_dir.join("reviewer.md"),
            "---\nname: reviewer\npre_launch_checks:\n  - vcs_conflicts\n---\nPrompt",
        ));

        let mut input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "invoke_subagent".to_owned(),
                args: json!({
                    "TypeName": "reviewer",
                    "Role": "Review auditor",
                    "Prompt": "Check workspace readiness"
                }),
            }),
            ..Default::default()
        };
        input.common.workspace_paths = vec![temp_dir.clone()];

        let res = evaluate_tool_invocation(&input);
        assert_eq!(res.decision, agy_common::models::HookDecision::Allow);

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
