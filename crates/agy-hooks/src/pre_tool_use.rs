//! Unified declarative `PreToolUse` hybrid execution engine.

use std::path::{Path, PathBuf};
use std::process::Command;
use agy_common::{CircuitBreakerLedger, HookDecision, PreToolResponse, PreToolUseInput, WorkspaceResolver};
use crate::config::{CustomCheck, HooksConfig, matches_pattern};
use crate::{conflicts, finisher_gate, manifest_guard, policy_gate, stdin};

/// Executes the unified `pre-tool-use` hook.
pub(crate) fn execute() {
    let input = stdin::read_stdin_payload::<PreToolUseInput>();
    let root = input
        .common
        .workspace_paths
        .first()
        .cloned()
        .unwrap_or_else(|| WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from(".")));

    let config = HooksConfig::load(&root);
    let response = evaluate_pre_tool_use(&input, &root, &config);

    if let Ok(json) = serde_json::to_string(&response) {
        println!("{json}");
    }
}

/// Evaluates a tool invocation input against the configured hybrid pipeline.
#[must_use]
pub(crate) fn evaluate_pre_tool_use(
    input: &PreToolUseInput,
    root: &Path,
    config: &HooksConfig,
) -> PreToolResponse {
    let Some(ref tc) = input.tool_call else {
        return PreToolResponse::allow();
    };

    let tool_name = tc.name.as_str();

    // 1. Evaluate native pipeline primitives in declared sequence
    for stage in &config.pre_tool_use.pipeline {
        match stage.as_str() {
            "circuit_breaker" => {
                let mut ledger = CircuitBreakerLedger::load(root);
                if config.circuit_breaker.max_strikes > 0 {
                    ledger.max_strikes = config.circuit_breaker.max_strikes;
                }
                if (ledger.tripped || ledger.consecutive_failures >= ledger.max_strikes)
                    && policy_gate::is_mutating_tool(tool_name, root)
                {
                    return PreToolResponse::ask(format!(
                        "🛑 CIRCUIT BREAKER TRIPPED ({} consecutive compilation failures):\n\
                         Last Error: {}\n\
                         Autonomous loop halted to prevent token burn and code regression.\n\
                         Confirm manual override to allow tool '{tool_name}'?",
                        ledger.consecutive_failures,
                        ledger.last_error_signature.as_deref().unwrap_or("Compilation error")
                    ));
                }
            }
            "manifest_guard" => {
                if let Some(resp) = manifest_guard::evaluate_manifest_guard(input, config) {
                    return resp;
                }
            }
            "subagent_admission" => {
                if tool_name == "invoke_subagent" {
                    let resp = finisher_gate::evaluate_tool_invocation(input);
                    if resp.decision != HookDecision::Allow {
                        return resp;
                    }
                }
            }
            "vcs_conflicts" => {
                if matches_pattern(tool_name, &config.pre_tool_use.vcs_conflicts_matcher)
                    && let Some(reason) = conflicts::detect_conflicts(root)
                {
                    return PreToolResponse::deny(reason);
                }
            }
            "tool_policy" | "permission_gate" => {
                if policy_gate::is_tool_gated_in_config(tool_name, root) {
                    return PreToolResponse::ask(format!(
                        "Policy Gate: Tool '{tool_name}' requires interactive operator confirmation before execution."
                    ));
                }
                if policy_gate::is_mutating_tool(tool_name, root) {
                    return PreToolResponse::ask(format!(
                        "Policy Gate: Tool '{tool_name}' applies automated filesystem or code modifications. Confirm execution?"
                    ));
                }
            }
            _ => {
                // Unknown native stage: ignored to ensure backward compatibility
            }
        }
    }

    // 2. Evaluate declarative custom checks
    for check in &config.pre_tool_use.custom_checks {
        if matches_pattern(tool_name, &check.matcher)
            && let Some(resp) = execute_custom_check(check, root)
        {
            return resp;
        }
    }

    PreToolResponse::allow()
}

fn execute_custom_check(check: &CustomCheck, root: &Path) -> Option<PreToolResponse> {
    let binary = check.command.first()?;
    let args = check.command.get(1..).unwrap_or(&[]);

    let mut cmd = Command::new(binary);
    cmd.args(args);
    cmd.current_dir(root);

    let output = cmd.output().ok()?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    if let Some(ref needle) = check.fail_if_output_contains
        && combined.contains(needle)
    {
        return Some(match check.decision {
            HookDecision::Ask => PreToolResponse::ask(check.reason.clone()),
            _ => PreToolResponse::deny(check.reason.clone()),
        });
    }

    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;
    use agy_common::models::ToolCallPayload;
    use serde_json::json;

    #[test]
    fn test_unified_read_only_tool_allowed() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "read_project_file".to_owned(),
                args: json!({"path": "src/lib.rs"}),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let res = evaluate_pre_tool_use(&input, Path::new("."), &config);
        assert_eq!(res.decision, HookDecision::Allow);
    }

    #[test]
    fn test_unified_mutating_tool_asks() {
        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "modify_code".to_owned(),
                args: json!({"path": "src/lib.rs"}),
            }),
            ..Default::default()
        };

        let config = HooksConfig::default();
        let res = evaluate_pre_tool_use(&input, Path::new("."), &config);
        assert_eq!(res.decision, HookDecision::Ask);
    }

    #[test]
    fn test_unified_custom_check_fails() {
        let mut config = HooksConfig::default();
        config.pre_tool_use.custom_checks.push(CustomCheck {
            name: "test_check".to_owned(),
            matcher: "read_project_file".to_owned(),
            command: vec!["echo".to_owned(), "DISK_FULL".to_owned()],
            fail_if_output_contains: Some("DISK_FULL".to_owned()),
            decision: HookDecision::Deny,
            reason: "Simulated custom check failure".to_owned(),
        });

        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "read_project_file".to_owned(),
                args: json!({"path": "src/lib.rs"}),
            }),
            ..Default::default()
        };

        let res = evaluate_pre_tool_use(&input, Path::new("."), &config);
        assert_eq!(res.decision, HookDecision::Deny);
        assert!(res.reason.unwrap().contains("Simulated custom check failure"));
    }
}
