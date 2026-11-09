//! Granular tool-level permission policy enforcement (`enforce-tool-policy`).

use std::path::Path;

use tkg_common::{
    CircuitBreakerLedger, FastHashMap, PreToolResponse, PreToolUseInput,
    tools::{RawToolsConfig, ToolDefinition},
};

use crate::stdin;

/// Executes the `enforce-tool-policy` command.
///
/// Inspects incoming `PreToolUseInput` on `stdin`. Evaluates whether the requested tool
/// requires human confirmation (`ask`) or is authorized for autonomous execution (`allow`).
pub(crate) fn execute() {
    let input = stdin::read_pre_tool_use_input();
    let root = input.common.resolve_workspace_root();

    let response = evaluate_policy(&input, &root);
    stdin::write_pre_tool_use_response(&response);
}

/// Evaluates a tool call against the granular security policy.
pub(crate) fn evaluate_policy(input: &PreToolUseInput, root: &Path) -> PreToolResponse {
    let Some(ref tc) = input.tool_call else {
        return PreToolResponse::allow();
    };

    let tool_name = tc.name.as_str();
    let tools_cfg = ParsedToolsConfig::load(root);

    // 0. Circuit Breaker Sentry: If consecutive failures exceed threshold, all mutating tools require explicit confirmation
    let mut ledger = CircuitBreakerLedger::load(root);
    let hooks_cfg = crate::config::HooksConfig::load(root);
    if hooks_cfg.circuit_breaker.max_strikes > 0 {
        ledger.max_strikes = hooks_cfg.circuit_breaker.max_strikes;
    }
    if (ledger.tripped || ledger.consecutive_failures >= ledger.max_strikes)
        && tools_cfg.is_mutating(tool_name)
    {
        return PreToolResponse::ask(format!(
            "🛑 CIRCUIT BREAKER TRIPPED ({} consecutive gate/verification failures):\n\
             Last Error/Failure: {}\n\
             Autonomous loop halted to prevent token burn and code regression.\n\
             Confirm manual override to allow tool '{tool_name}'?",
            ledger.consecutive_failures,
            ledger
                .last_error_signature
                .as_deref()
                .unwrap_or("Verification error")
        ));
    }

    // 1. Check if tool is explicitly declared as requiring permission in tools.toml
    if tools_cfg.requires_permission_gate(tool_name) {
        return PreToolResponse::ask(format!(
            "Policy Gate: Tool '{tool_name}' requires interactive operator confirmation before execution."
        ));
    }

    // 2. Mutating tools that alter source code or repository manifests
    if tools_cfg.is_mutating(tool_name) {
        return PreToolResponse::ask(format!(
            "Policy Gate: Tool '{tool_name}' applies automated filesystem or code modifications. Confirm execution?"
        ));
    }

    // 3. All other verification and inspection tools are allowed autonomously
    PreToolResponse::allow()
}

/// Resolved map of tool definitions loaded from `tools.toml` via [`tkg_common::tools`].
#[derive(Debug, Default, Clone)]
pub(crate) struct ParsedToolsConfig {
    pub(crate) tools: FastHashMap<String, ToolDefinition>,
}

impl ParsedToolsConfig {
    /// Parses and resolves a `tools.toml` content string into structured tool definitions.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn from_str(content: &str) -> Self {
        let raw = RawToolsConfig::from_toml_str(content);
        let tools = tkg_common::tools::resolve(&raw)
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect();
        Self { tools }
    }

    /// Loads, merges, and resolves `tools.toml` configuration across candidate locations.
    #[must_use]
    pub(crate) fn load(root: &Path) -> Self {
        Self {
            tools: RawToolsConfig::load_resolved_map(root),
        }
    }

    /// Checks whether a tool is mutating.
    #[must_use]
    pub(crate) fn is_mutating(&self, tool_name: &str) -> bool {
        self.tools
            .get(tool_name)
            .is_some_and(ToolDefinition::mutating)
    }

    /// Checks whether a tool requires interactive permission confirmation.
    #[must_use]
    pub(crate) fn requires_permission_gate(&self, tool_name: &str) -> bool {
        self.tools
            .get(tool_name)
            .is_some_and(ToolDefinition::require_permission_gate)
    }

    /// Checks if a tool resolves a boolean attribute to true across concrete, base, and category tiers.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn is_tool_attribute(&self, tool_name: &str, attr: &str) -> bool {
        let Some(tool) = self.tools.get(tool_name) else {
            return false;
        };
        match attr {
            "mutating" => tool.mutating(),
            "tracks_circuit_breaker" => tool.tracks_circuit_breaker(),
            "require_permission_gate" => tool.require_permission_gate(),
            _ => false,
        }
    }

    /// Evaluates whether a tool requires a specific gate after category inheritance and skip filtering.
    #[must_use]
    pub(crate) fn tool_requires_gate(&self, tool_name: &str, gate_name: &str) -> bool {
        self.tools
            .get(tool_name)
            .is_some_and(|t| t.requires_gate(gate_name))
    }
}

/// Checks whether a tool is declared as a mutating tool in declarative configuration.
pub(crate) fn is_mutating_tool(tool_name: &str, root: &Path) -> bool {
    ParsedToolsConfig::load(root).is_mutating(tool_name)
}

/// Evaluates whether a tool requires a specific gate before execution.
#[cfg(test)]
#[must_use]
pub(crate) fn tool_requires_gate(tool_name: &str, root: &Path, gate_name: &str) -> bool {
    ParsedToolsConfig::load(root).tool_requires_gate(tool_name, gate_name)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use serde_json::json;
    use tkg_common::models::{HookDecision, ToolCallPayload};

    use super::*;

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
                name: "cargo_clippy_fix".to_owned(),
                args: json!({}),
            }),
            ..Default::default()
        };

        let res = evaluate_policy(&input, Path::new("."));
        assert_eq!(res.decision, HookDecision::Ask);
    }

    #[test]
    fn test_circuit_breaker_tripped_triggers_ask() {
        let temp_dir = std::env::temp_dir().join(format!(
            "hook_cb_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&temp_dir));
        drop(std::fs::write(
            temp_dir.join("tools.toml"),
            "[tools.modify_code]\nmutating = true\n",
        ));

        let mut ledger = CircuitBreakerLedger::new("worker_agent");
        drop(ledger.record_failure(
            &temp_dir,
            Some("worker_agent"),
            Some("quick_check"),
            Some("E0502"),
            Some("borrow error"),
        ));
        drop(ledger.record_failure(
            &temp_dir,
            Some("worker_agent"),
            Some("quick_check"),
            Some("E0502"),
            Some("borrow error"),
        ));
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
        let temp_dir = std::env::temp_dir().join(format!(
            "hook_decl_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
        assert!(
            res.reason
                .unwrap()
                .contains("applies automated filesystem or code modifications")
        );

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
        let config = ParsedToolsConfig::from_str(sample);
        assert!(config.is_tool_attribute("child_tool", "mutating"));
        assert!(!config.is_tool_attribute("child_tool", "require_permission_gate"));
    }

    #[test]
    fn test_category_inheritance_and_gates() {
        let sample = r#"
[categories.compilation]
tracks_circuit_breaker = true
require_gates = ["vcs_conflicts"]

[base_tools.cargo_runner]
category = "compilation"
action = "static_command"

[tools.clippy]
inherit = "base_tools.cargo_runner"
require_gates = ["custom_lint"]

[tools.special_check]
inherit = "base_tools.cargo_runner"
skip_gates = ["vcs_conflicts"]

[tools.skip_all]
category = "compilation"
skip_gates = ["*"]

[tools.unrelated]
action = "file_read"
"#;
        let config = ParsedToolsConfig::from_str(sample);

        // Tool 1: clippy inherits category "compilation" from base_tools.cargo_runner
        assert!(config.is_tool_attribute("clippy", "tracks_circuit_breaker"));
        assert!(config.tool_requires_gate("clippy", "vcs_conflicts"));
        assert!(config.tool_requires_gate("clippy", "custom_lint"));
        assert!(!config.tool_requires_gate("clippy", "other_gate"));

        // Tool 2: special_check skips vcs_conflicts
        assert!(config.is_tool_attribute("special_check", "tracks_circuit_breaker"));
        assert!(!config.tool_requires_gate("special_check", "vcs_conflicts"));

        // Tool 3: skip_all skips all gates
        assert!(!config.tool_requires_gate("skip_all", "vcs_conflicts"));

        // Tool 4: unrelated has no gates
        assert!(!config.tool_requires_gate("unrelated", "vcs_conflicts"));
    }
}
