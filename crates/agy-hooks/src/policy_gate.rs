//! Granular tool-level permission policy enforcement (`enforce-tool-policy`).

use std::path::{Path, PathBuf};

use agy_common::{
    CircuitBreakerLedger, FastHashMap, PreToolResponse, PreToolUseInput, WorkspaceResolver,
};

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
        .unwrap_or_else(|| {
            WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."))
        });

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
    let tools_cfg = ParsedToolsConfig::load(root);

    // 0. Circuit Breaker Sentry: If consecutive failures exceed threshold, all mutating tools require explicit confirmation
    let mut ledger = CircuitBreakerLedger::load(root);
    let hooks_cfg = crate::config::HooksConfig::load(root);
    if hooks_cfg.circuit_breaker.max_strikes > 0 {
        ledger.max_strikes = hooks_cfg.circuit_breaker.max_strikes;
    }
    if (ledger.tripped || ledger.consecutive_failures >= ledger.max_strikes)
        && tools_cfg.is_tool_attribute(tool_name, "mutating")
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
    if tools_cfg.is_tool_attribute(tool_name, "require_permission_gate") {
        return PreToolResponse::ask(format!(
            "Policy Gate: Tool '{tool_name}' requires interactive operator confirmation before execution."
        ));
    }

    // 2. Mutating tools that alter source code or repository manifests
    if tools_cfg.is_tool_attribute(tool_name, "mutating") {
        return PreToolResponse::ask(format!(
            "Policy Gate: Tool '{tool_name}' applies automated filesystem or code modifications. Confirm execution?"
        ));
    }

    // 3. All other verification and inspection tools are allowed autonomously
    PreToolResponse::allow()
}

/// Raw representation of a tool, base tool, or category entry parsed from `tools.toml`.
#[derive(Debug, Default, Clone)]
pub(crate) struct ParsedToolEntry {
    pub(crate) inherit: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) mutating: Option<bool>,
    pub(crate) tracks_circuit_breaker: Option<bool>,
    pub(crate) require_permission_gate: Option<bool>,
    pub(crate) require_gates: Vec<String>,
    pub(crate) skip_gates: Vec<String>,
}

/// Lightweight container for tools, base tools, and category templates parsed from `tools.toml`.
#[derive(Debug, Default, Clone)]
pub(crate) struct ParsedToolsConfig {
    pub(crate) tools: FastHashMap<String, ParsedToolEntry>,
    pub(crate) base_tools: FastHashMap<String, ParsedToolEntry>,
    pub(crate) categories: FastHashMap<String, ParsedToolEntry>,
}

fn parse_tool_entry(table: &toml::Table) -> ParsedToolEntry {
    let inherit = table
        .get("inherit")
        .and_then(toml::Value::as_str)
        .map(|s| s.strip_prefix("base_tools.").unwrap_or(s).to_owned());
    let category = table
        .get("category")
        .and_then(toml::Value::as_str)
        .map(|s| s.strip_prefix("categories.").unwrap_or(s).to_owned());
    let mutating = table.get("mutating").and_then(toml::Value::as_bool);
    let tracks_circuit_breaker = table
        .get("tracks_circuit_breaker")
        .and_then(toml::Value::as_bool);
    let require_permission_gate = table
        .get("require_permission_gate")
        .and_then(toml::Value::as_bool);
    let require_gates = table
        .get("require_gates")
        .and_then(toml::Value::as_array)
        .map_or_default(|arr| {
            arr.iter()
                .filter_map(toml::Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        });
    let skip_gates = table
        .get("skip_gates")
        .and_then(toml::Value::as_array)
        .map_or_default(|arr| {
            arr.iter()
                .filter_map(toml::Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        });

    ParsedToolEntry {
        inherit,
        category,
        mutating,
        tracks_circuit_breaker,
        require_permission_gate,
        require_gates,
        skip_gates,
    }
}

impl ParsedToolsConfig {
    /// Parses a `tools.toml` content string into structured tool and category definitions.
    #[must_use]
    pub(crate) fn from_str(content: &str) -> Self {
        let Ok(table) = content.parse::<toml::Table>() else {
            return Self::default();
        };

        let mut config = Self::default();

        if let Some(tools_table) = table.get("tools").and_then(toml::Value::as_table) {
            for (name, val) in tools_table {
                if let Some(t) = val.as_table() {
                    config.tools.insert(name.clone(), parse_tool_entry(t));
                }
            }
        }

        if let Some(base_table) = table.get("base_tools").and_then(toml::Value::as_table) {
            for (name, val) in base_table {
                if let Some(t) = val.as_table() {
                    config.base_tools.insert(name.clone(), parse_tool_entry(t));
                }
            }
        }

        if let Some(cat_table) = table.get("categories").and_then(toml::Value::as_table) {
            for (name, val) in cat_table {
                if let Some(t) = val.as_table() {
                    config.categories.insert(name.clone(), parse_tool_entry(t));
                }
            }
        }

        config
    }

    /// Checks if a tool resolves a boolean attribute to true across concrete, base, and category tiers.
    #[must_use]
    pub(crate) fn is_tool_attribute(&self, tool_name: &str, attr: &str) -> bool {
        let Some(tool) = self.tools.get(tool_name) else {
            return false;
        };

        let base = tool.inherit.as_deref().and_then(|b| self.base_tools.get(b));
        let cat_name = tool
            .category
            .as_deref()
            .or_else(|| base?.category.as_deref());
        let category = cat_name.and_then(|c| self.categories.get(c));

        match attr {
            "mutating" => tool
                .mutating
                .or_else(|| base?.mutating)
                .or_else(|| category?.mutating)
                .unwrap_or(false),
            "tracks_circuit_breaker" => tool
                .tracks_circuit_breaker
                .or_else(|| base?.tracks_circuit_breaker)
                .or_else(|| category?.tracks_circuit_breaker)
                .unwrap_or(false),
            "require_permission_gate" => tool
                .require_permission_gate
                .or_else(|| base?.require_permission_gate)
                .or_else(|| category?.require_permission_gate)
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Evaluates whether a tool requires a specific gate after category inheritance and skip filtering.
    #[must_use]
    pub(crate) fn tool_requires_gate(&self, tool_name: &str, gate_name: &str) -> bool {
        let Some(tool) = self.tools.get(tool_name) else {
            return false;
        };

        let base = tool.inherit.as_deref().and_then(|b| self.base_tools.get(b));
        let cat_name = tool
            .category
            .as_deref()
            .or_else(|| base?.category.as_deref());
        let category = cat_name.and_then(|c| self.categories.get(c));

        let mut gates = Vec::new();
        if let Some(cat) = category {
            for g in &cat.require_gates {
                if !gates
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(g))
                {
                    gates.push(g.clone());
                }
            }
        }
        if let Some(b) = base {
            for g in &b.require_gates {
                if !gates
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(g))
                {
                    gates.push(g.clone());
                }
            }
        }
        for g in &tool.require_gates {
            if !gates
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(g))
            {
                gates.push(g.clone());
            }
        }

        let mut skip_gates = Vec::new();
        if let Some(cat) = category {
            skip_gates.extend(cat.skip_gates.clone());
        }
        if let Some(b) = base {
            skip_gates.extend(b.skip_gates.clone());
        }
        skip_gates.extend(tool.skip_gates.clone());

        if skip_gates
            .iter()
            .any(|s| s == "*" || s.eq_ignore_ascii_case("all"))
        {
            return false;
        }

        gates.retain(|g| !skip_gates.iter().any(|s| s.eq_ignore_ascii_case(g)));

        gates.iter().any(|g| g.eq_ignore_ascii_case(gate_name))
    }

    /// Loads and merges `tools.toml` configuration by searching candidate locations.
    #[must_use]
    pub(crate) fn load(root: &Path) -> Self {
        let ws_root =
            WorkspaceResolver::find_root(Some(root)).unwrap_or_else(|_| root.to_path_buf());
        let mut candidates = vec![
            root.join("tools.toml"),
            root.join("plugins/agy-suite/tools.toml"),
            ws_root.join("tools.toml"),
            ws_root.join("plugins/agy-suite/tools.toml"),
        ];

        if let Ok(home) = std::env::var("HOME") {
            candidates
                .push(PathBuf::from(home).join(".gemini/config/plugins/agy-suite/tools.toml"));
        }

        let mut merged = Self::default();
        // Load in reverse order so higher-priority candidates overwrite lower-priority ones
        for path in candidates.into_iter().rev() {
            if path.is_file()
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                let parsed = Self::from_str(&content);
                merged.tools.extend(parsed.tools);
                merged.base_tools.extend(parsed.base_tools);
                merged.categories.extend(parsed.categories);
            }
        }
        merged
    }
}

/// Checks whether a tool is declared as a mutating tool in declarative configuration.
pub(crate) fn is_mutating_tool(tool_name: &str, root: &Path) -> bool {
    ParsedToolsConfig::load(root).is_tool_attribute(tool_name, "mutating")
}

/// Inspects tools.toml (if present) for `require_permission_gate = true`.
pub(crate) fn is_tool_gated_in_config(tool_name: &str, root: &Path) -> bool {
    ParsedToolsConfig::load(root).is_tool_attribute(tool_name, "require_permission_gate")
}

/// Evaluates whether a tool requires a specific gate before execution.
#[must_use]
pub(crate) fn tool_requires_gate(tool_name: &str, root: &Path, gate_name: &str) -> bool {
    ParsedToolsConfig::load(root).tool_requires_gate(tool_name, gate_name)
}

/// Evaluates whether a tool failure tracks strikes against the circuit breaker ledger.
#[must_use]
#[allow(
    dead_code,
    reason = "Declarative query utility for circuit breaker policy"
)]
pub(crate) fn tool_tracks_circuit_breaker(tool_name: &str, root: &Path) -> bool {
    ParsedToolsConfig::load(root).is_tool_attribute(tool_name, "tracks_circuit_breaker")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use agy_common::models::{HookDecision, ToolCallPayload};
    use serde_json::json;

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
