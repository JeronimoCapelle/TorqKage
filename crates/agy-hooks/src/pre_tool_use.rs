//! Unified declarative `PreToolUse` hybrid execution engine.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use agy_common::{
    CircuitBreakerLedger, HookDecision, PreToolResponse, PreToolUseInput, WorkspaceResolver,
};

use crate::{
    config::{CustomCheck, HooksConfig, matches_pattern},
    conflicts, manifest_guard, policy_gate, stdin, subagent_gate,
};

/// Executes the unified `pre-tool-use` hook.
pub(crate) fn execute() {
    let input = stdin::read_stdin_payload::<PreToolUseInput>();
    let root = input
        .common
        .workspace_paths
        .first()
        .cloned()
        .unwrap_or_else(|| {
            WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."))
        });

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
                if config.circuit_breaker.enabled {
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
                            ledger
                                .last_error_signature
                                .as_deref()
                                .unwrap_or("Compilation error")
                        ));
                    }
                }
            },
            "manifest_guard" => {
                if let Some(resp) = manifest_guard::evaluate_manifest_guard(input, root, config) {
                    return resp;
                }
            },
            "path_policies" => {
                if let Some(resp) = evaluate_path_policies(input, root) {
                    return resp;
                }
            },
            "subagent_admission" => {
                if tool_name == "invoke_subagent" {
                    let resp = subagent_gate::evaluate_tool_invocation(input);
                    if resp.decision != HookDecision::Allow {
                        return resp;
                    }
                }
            },
            "vcs_conflicts" => {
                if policy_gate::tool_requires_gate(tool_name, root, "vcs_conflicts")
                    && let Some(reason) = conflicts::detect_conflicts(root)
                {
                    return PreToolResponse::deny(reason);
                }
            },
            "tool_policy" => {
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
            },
            _ => {
                // Non-native stages continue to declarative custom checks
            },
        }
    }

    // 2. Evaluate declarative custom checks
    for check in &config.pre_tool_use.custom_checks {
        let matches = policy_gate::tool_requires_gate(tool_name, root, &check.name)
            || (!check.matcher.is_empty() && matches_pattern(tool_name, &check.matcher));

        if matches && let Some(resp) = execute_custom_check(check, root) {
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

    let output = agy_common::SubprocessSupervisor::run_supervised(
        cmd,
        agy_common::DEFAULT_QUICK_COMMAND_TIMEOUT_SECS,
    )
    .ok()?;
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

fn resolve_active_agent(input: &PreToolUseInput, root: &Path) -> Option<String> {
    if let Ok(agent) = std::env::var("AGY_AGENT").or_else(|_| std::env::var("AGENT")) {
        let trimmed = agent.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    let paths = agy_common::state::AgyStatePaths::new(root);
    let json_path = paths.active_agent_file();
    if json_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&json_path)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(name) = val.get("agent").and_then(serde_json::Value::as_str)
    {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    let txt_path = paths.state_dir().join("active_agent");
    if txt_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&txt_path)
    {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    if let Some(ref model) = input.common.model_name {
        let trimmed = model.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    None
}

/// Evaluates agent directory boundary guards and path policies on mutating tool calls.
#[must_use]
#[allow(
    clippy::option_if_let_else,
    clippy::collapsible_if,
    reason = "Deterministic priority resolution of tool argument payloads"
)]
pub(crate) fn evaluate_path_policies(
    input: &PreToolUseInput,
    root: &Path,
) -> Option<PreToolResponse> {
    let tc = input.tool_call.as_ref()?;
    let tool_name = tc.name.as_str();

    let candidates = manifest_guard::extract_candidate_paths(&tc.args);
    if candidates.is_empty() {
        return None;
    }

    let is_mutation = policy_gate::is_mutating_tool(tool_name, root)
        || manifest_guard::is_default_fallback_mutation_tool(tool_name)
        || tc.args.get("content").is_some()
        || tc.args.get("Content").is_some()
        || tc.args.get("patch").is_some()
        || tc.args.get("target").is_some()
        || tc.args.get("Target").is_some();

    if !is_mutation {
        return None;
    }

    let active_agent = resolve_active_agent(input, root);
    let agent_guard = active_agent.as_deref().and_then(|agent| {
        let manifest_path = agy_common::find_agent_manifest_path(root, agent)?;
        let content = std::fs::read_to_string(manifest_path).ok()?;
        let frontmatter = agy_common::AgentFrontmatter::parse(&content).ok()?;
        Some(frontmatter.to_guard())
    });

    let project = agy_common::ProjectConfig::detect_or_load(root);

    for cand in candidates {
        let full_path = if Path::new(cand).is_absolute() {
            PathBuf::from(cand)
        } else {
            root.join(cand)
        };

        // 1. Check directory confinement
        if let Err(err) = agy_common::check_agent_guard_direct(
            root,
            &full_path,
            active_agent.as_deref(),
            agent_guard.as_ref(),
        ) {
            return Some(PreToolResponse::deny(err));
        }

        // 2. Check path policies (content filter)
        let original = std::fs::read_to_string(&full_path).unwrap_or_default();
        let new_content_opt = if let Some(c) = tc
            .args
            .get("content")
            .or_else(|| tc.args.get("Content"))
            .and_then(serde_json::Value::as_str)
        {
            Some(c.to_owned())
        } else if let Some(target) = tc
            .args
            .get("target")
            .or_else(|| tc.args.get("Target"))
            .and_then(serde_json::Value::as_str)
            && let Some(replacement) = tc
                .args
                .get("replacement")
                .or_else(|| tc.args.get("Replacement"))
                .and_then(serde_json::Value::as_str)
        {
            Some(original.replace(target, replacement))
        } else {
            None
        };

        if let Some(ref new_content) = new_content_opt {
            if let Err(err) = agy_common::check_agent_path_policies_direct(
                root,
                &full_path,
                &original,
                new_content,
                active_agent.as_deref(),
                agent_guard.as_ref(),
                Some(&project),
            ) {
                return Some(PreToolResponse::deny(err));
            }
        }
    }

    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use agy_common::models::ToolCallPayload;
    use serde_json::json;

    use super::*;

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
        assert!(
            res.reason
                .unwrap()
                .contains("Simulated custom check failure")
        );
    }

    #[test]
    fn test_explicit_category_gate_triggers_vcs_conflicts_and_custom_checks() {
        let temp_dir = std::env::temp_dir().join(format!(
            "gate_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&temp_dir));
        drop(std::fs::write(
            temp_dir.join("tools.toml"),
            r#"
[categories.compilation]
require_gates = ["vcs_conflicts", "custom_sentinel"]

[tools.my_compiler]
category = "compilation"

[tools.my_unfettered_tool]
category = "compilation"
skip_gates = ["vcs_conflicts", "custom_sentinel"]
"#,
        ));

        // Verify policy_gate resolves category gates & skip_gates from tools.toml on disk
        assert!(policy_gate::tool_requires_gate(
            "my_compiler",
            &temp_dir,
            "vcs_conflicts"
        ));
        assert!(policy_gate::tool_requires_gate(
            "my_compiler",
            &temp_dir,
            "custom_sentinel"
        ));
        assert!(!policy_gate::tool_requires_gate(
            "my_unfettered_tool",
            &temp_dir,
            "vcs_conflicts"
        ));
        assert!(!policy_gate::tool_requires_gate(
            "my_unfettered_tool",
            &temp_dir,
            "custom_sentinel"
        ));

        // PreToolUse custom check triggers purely via declarative require_gates
        let mut config = HooksConfig::default();
        config.pre_tool_use.custom_checks.push(CustomCheck {
            name: "custom_sentinel".to_owned(),
            matcher: String::new(),
            command: vec!["echo".to_owned(), "GATE_TRIGGERED".to_owned()],
            fail_if_output_contains: Some("GATE_TRIGGERED".to_owned()),
            decision: HookDecision::Deny,
            reason: "Custom sentinel gate blocked execution".to_owned(),
        });

        let input_gated = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "my_compiler".to_owned(),
                args: json!({}),
            }),
            ..Default::default()
        };

        let res_gated = evaluate_pre_tool_use(&input_gated, &temp_dir, &config);
        assert_eq!(res_gated.decision, HookDecision::Deny);
        assert!(
            res_gated
                .reason
                .unwrap()
                .contains("Custom sentinel gate blocked execution")
        );

        let input_skipped = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "my_unfettered_tool".to_owned(),
                args: json!({}),
            }),
            ..Default::default()
        };

        let res_skipped = evaluate_pre_tool_use(&input_skipped, &temp_dir, &config);
        assert_eq!(res_skipped.decision, HookDecision::Allow);

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_pre_tool_use_path_policies_and_agent_frontmatter_guards() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("agy_hook_policy_test_{unique_id}"));
        drop(std::fs::create_dir_all(
            temp_dir.join("plugins/agy-suite/agents"),
        ));
        drop(std::fs::create_dir_all(temp_dir.join("src")));

        let scoped_agent_manifest = "---\nname: comments_restricted_agent\nallowed_dirs:\n  - docs\n  - src\npath_policies:\n  - path: src\n    filter: comments_only\n---\n";
        std::fs::write(
            temp_dir.join("plugins/agy-suite/agents/comments_restricted_agent.md"),
            scoped_agent_manifest,
        )
        .unwrap();

        let orig_code = "pub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n";
        std::fs::write(temp_dir.join("src/lib.rs"), orig_code).unwrap();

        let config = HooksConfig::default();

        // 1. Tool targeting path outside allowed_dirs (e.g. tests/) gets DENIED
        let input_disallowed = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "custom_writer".to_owned(),
                args: json!({
                    "path": "tests/test.rs",
                    "content": "fn test() {}"
                }),
            }),
            common: agy_common::models::CommonHookPayload {
                model_name: Some("comments_restricted_agent".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        };
        let res_disallowed = evaluate_pre_tool_use(&input_disallowed, &temp_dir, &config);
        assert_eq!(res_disallowed.decision, HookDecision::Deny);
        assert!(
            res_disallowed
                .reason
                .unwrap()
                .contains("Security policy violation")
        );

        // 2. Tool attempting code tamper in src/ gets DENIED
        let input_tamper = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "edit_file".to_owned(),
                args: json!({
                    "path": "src/lib.rs",
                    "target": "a + b",
                    "replacement": "a + b + 1"
                }),
            }),
            common: agy_common::models::CommonHookPayload {
                model_name: Some("comments_restricted_agent".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        };
        let res_tamper = evaluate_pre_tool_use(&input_tamper, &temp_dir, &config);
        assert_eq!(res_tamper.decision, HookDecision::Deny);
        assert!(
            res_tamper
                .reason
                .unwrap()
                .contains("Security policy violation")
        );

        // 3. Tool adding valid doc comments gets ALLOWED
        let input_docs = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "edit_file".to_owned(),
                args: json!({
                    "path": "src/lib.rs",
                    "target": "pub fn add",
                    "replacement": "/// Adds two numbers.\npub fn add"
                }),
            }),
            common: agy_common::models::CommonHookPayload {
                model_name: Some("comments_restricted_agent".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        };
        let res_docs = evaluate_pre_tool_use(&input_docs, &temp_dir, &config);
        assert_eq!(res_docs.decision, HookDecision::Allow);

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
