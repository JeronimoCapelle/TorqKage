//! Granular tool-level permission policy enforcement (`enforce-tool-policy`).

use std::path::{Path, PathBuf};
use agy_common::{CircuitBreakerLedger, PreToolResponse, PreToolUseInput, WorkspaceResolver};

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

    // 0. Circuit Breaker Sentry: If consecutive failures exceed threshold, all mutating tools require explicit confirmation
    let mut ledger = CircuitBreakerLedger::load(root);
    let hooks_cfg = crate::config::HooksConfig::load(root);
    if hooks_cfg.circuit_breaker.max_strikes > 0 {
        ledger.max_strikes = hooks_cfg.circuit_breaker.max_strikes;
    }
    if (ledger.tripped || ledger.consecutive_failures >= ledger.max_strikes)
        && is_mutating_tool(tool_name, root)
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

    // 1. Check if tool is explicitly declared as requiring permission in tools.toml
    if is_tool_gated_in_config(tool_name, root) {
        return PreToolResponse::ask(format!(
            "Policy Gate: Tool '{tool_name}' requires interactive operator confirmation before execution."
        ));
    }

    // 2. Mutating tools that alter source code or repository manifests
    if is_mutating_tool(tool_name, root) {
        return PreToolResponse::ask(format!(
            "Policy Gate: Tool '{tool_name}' applies automated filesystem or code modifications. Confirm execution?"
        ));
    }

    // 3. All other verification and inspection tools are allowed autonomously
    PreToolResponse::allow()
}

/// Checks whether a tool is declared as a mutating tool in declarative configuration.
pub(crate) fn is_mutating_tool(tool_name: &str, root: &Path) -> bool {
    check_tool_attribute_in_config(tool_name, root, "mutating")
}

/// Inspects tools.toml (if present) for `require_permission_gate = true`.
pub(crate) fn is_tool_gated_in_config(tool_name: &str, root: &Path) -> bool {
    check_tool_attribute_in_config(tool_name, root, "require_permission_gate")
}

/// Inspects tools.toml files across workspace and plugin installations for a boolean flag.
fn check_tool_attribute_in_config(tool_name: &str, root: &Path, attribute: &str) -> bool {
    let ws_root = WorkspaceResolver::find_root(Some(root)).unwrap_or_else(|_| root.to_path_buf());
    let mut candidates = vec![
        root.join("tools.toml"),
        root.join("plugins/agy-suite/tools.toml"),
        ws_root.join("tools.toml"),
        ws_root.join("plugins/agy-suite/tools.toml"),
    ];

    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".gemini/config/plugins/agy-suite/tools.toml"));
    }

    for path in candidates {
        if path.is_file()
            && let Ok(content) = std::fs::read_to_string(&path)
            && check_section_attribute(&content, &format!("[tools.{tool_name}]"), attribute)
        {
            return true;
        }
    }

    false
}

fn check_section_attribute(content: &str, section_header: &str, attribute: &str) -> bool {
    let mut in_section = false;
    let mut inherit_target = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section {
            if trimmed.starts_with('[') {
                break;
            }
            if let Some((key, val)) = trimmed.split_once('=') {
                let k = key.trim();
                let v = val.trim().trim_matches(['"', '\'']);
                if k == attribute && v == "true" {
                    return true;
                }
                if k == "inherit" {
                    let clean_target = v.strip_prefix("base_tools.").unwrap_or(v);
                    inherit_target = Some(clean_target.to_owned());
                }
            }
        }
    }

    if let Some(base_name) = inherit_target {
        return check_section_attribute(content, &format!("[base_tools.{base_name}]"), attribute);
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
                name: "modify_code".to_owned(),
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

    #[test]
    fn test_circuit_breaker_tripped_triggers_ask() {
        let temp_dir = std::env::temp_dir().join(format!("hook_cb_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        drop(std::fs::create_dir_all(&temp_dir));
        drop(std::fs::write(
            temp_dir.join("tools.toml"),
            "[tools.modify_code]\nmutating = true\n",
        ));

        let mut ledger = CircuitBreakerLedger::new("coder");
        drop(ledger.record_failure(&temp_dir, Some("coder"), Some("run_quick_check"), Some("E0502"), Some("borrow error")));
        drop(ledger.record_failure(&temp_dir, Some("coder"), Some("run_quick_check"), Some("E0502"), Some("borrow error")));
        assert!(ledger.tripped);

        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "modify_code".to_owned(),
                args: json!({"path": "src/lib.rs", "diff": "..."}),
            }),
            ..Default::default()
        };

        let res = evaluate_policy(&input, &temp_dir);
        assert_eq!(res.decision, HookDecision::Ask);
        assert!(res.reason.unwrap().contains("CIRCUIT BREAKER TRIPPED"));

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_declarative_mutating_tool_triggers_ask() {
        let temp_dir = std::env::temp_dir().join(format!("hook_decl_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        drop(std::fs::create_dir_all(&temp_dir));
        drop(std::fs::write(
            temp_dir.join("tools.toml"),
            "[tools.custom_db_migrate]\nmutating = true\naction = \"static_command\"\n",
        ));

        let input = PreToolUseInput {
            tool_call: Some(ToolCallPayload {
                name: "custom_db_migrate".to_owned(),
                args: json!({}),
            }),
            ..Default::default()
        };

        let res = evaluate_policy(&input, &temp_dir);
        assert_eq!(res.decision, HookDecision::Ask);
        assert!(res.reason.unwrap().contains("applies automated filesystem or code modifications"));

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_inherited_mutating_tool_triggers_ask() {
        let sample = r#"
[base_tools.base_mutator]
mutating = true

[tools.child_tool]
inherit = "base_tools.base_mutator"
action = "static_command"
"#;
        assert!(check_section_attribute(sample, "[tools.child_tool]", "mutating"));
        assert!(!check_section_attribute(sample, "[tools.child_tool]", "require_permission_gate"));
    }
}
