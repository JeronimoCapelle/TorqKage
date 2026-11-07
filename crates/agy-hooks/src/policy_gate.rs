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

impl ParsedToolsConfig {
    /// Parses a `tools.toml` content string into structured tool and category definitions.
    #[must_use]
    pub(crate) fn from_str(content: &str) -> Self {
        let mut config = Self::default();
        let mut current_section: Option<(&str, String)> = None;
        let mut lines = content.lines();

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(inside) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                if let Some((kind, name)) = inside.split_once('.') {
                    let k = kind.trim();
                    let n = name.trim().to_owned();
                    match k {
                        "tools" => current_section = Some(("tools", n)),
                        "base_tools" => current_section = Some(("base_tools", n)),
                        "categories" => current_section = Some(("categories", n)),
                        _ => current_section = None,
                    }
                } else {
                    current_section = None;
                }
                continue;
            }

            let Some((kind, ref section_name)) = current_section else {
                continue;
            };

            let Some((key_part, val_part)) = trimmed.split_once('=') else {
                continue;
            };

            let key = key_part.trim();
            let mut val_str = val_part.trim().to_owned();

            if val_str.starts_with('[') && !val_str.contains(']') {
                for next_line in lines.by_ref() {
                    let next_trimmed = next_line.trim();
                    val_str.push(' ');
                    val_str.push_str(next_trimmed);
                    if next_trimmed.contains(']') {
                        break;
                    }
                }
            }

            let clean_val = strip_trailing_comment(&val_str);

            let entry = match kind {
                "tools" => config.tools.entry(section_name.clone()).or_default(),
                "base_tools" => config.base_tools.entry(section_name.clone()).or_default(),
                "categories" => config.categories.entry(section_name.clone()).or_default(),
                _ => continue,
            };

            match key {
                "inherit" => {
                    let clean = clean_val.trim().trim_matches(['"', '\'']);
                    let target = clean.strip_prefix("base_tools.").unwrap_or(clean);
                    entry.inherit = Some(target.to_owned());
                }
                "category" => {
                    let clean = clean_val.trim().trim_matches(['"', '\'']);
                    let target = clean.strip_prefix("categories.").unwrap_or(clean);
                    entry.category = Some(target.to_owned());
                }
                "mutating" => {
                    entry.mutating = Some(clean_val.trim() == "true");
                }
                "tracks_circuit_breaker" => {
                    entry.tracks_circuit_breaker = Some(clean_val.trim() == "true");
                }
                "require_permission_gate" => {
                    entry.require_permission_gate = Some(clean_val.trim() == "true");
                }
                "require_gates" => {
                    entry.require_gates = parse_string_array(&clean_val);
                }
                "skip_gates" => {
                    entry.skip_gates = parse_string_array(&clean_val);
                }
                _ => {}
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
        let cat_name = tool.category.as_deref().or_else(|| base?.category.as_deref());
        let category = cat_name.and_then(|c| self.categories.get(c));

        match attr {
            "mutating" => tool.mutating
                .or_else(|| base?.mutating)
                .or_else(|| category?.mutating)
                .unwrap_or(false),
            "tracks_circuit_breaker" => tool.tracks_circuit_breaker
                .or_else(|| base?.tracks_circuit_breaker)
                .or_else(|| category?.tracks_circuit_breaker)
                .unwrap_or(false),
            "require_permission_gate" => tool.require_permission_gate
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
        let cat_name = tool.category.as_deref().or_else(|| base?.category.as_deref());
        let category = cat_name.and_then(|c| self.categories.get(c));

        let mut gates = Vec::new();
        if let Some(cat) = category {
            for g in &cat.require_gates {
                if !gates.iter().any(|existing: &String| existing.eq_ignore_ascii_case(g)) {
                    gates.push(g.clone());
                }
            }
        }
        if let Some(b) = base {
            for g in &b.require_gates {
                if !gates.iter().any(|existing: &String| existing.eq_ignore_ascii_case(g)) {
                    gates.push(g.clone());
                }
            }
        }
        for g in &tool.require_gates {
            if !gates.iter().any(|existing: &String| existing.eq_ignore_ascii_case(g)) {
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

        if skip_gates.iter().any(|s| s == "*" || s.eq_ignore_ascii_case("all")) {
            return false;
        }

        gates.retain(|g| !skip_gates.iter().any(|s| s.eq_ignore_ascii_case(g)));

        gates.iter().any(|g| g.eq_ignore_ascii_case(gate_name))
    }
}

fn strip_trailing_comment(s: &str) -> String {
    let mut in_quote = false;
    let mut quote_char = '"';
    let mut out = String::new();
    for c in s.chars() {
        if in_quote {
            if c == quote_char {
                in_quote = false;
            }
            out.push(c);
        } else if c == '"' || c == '\'' {
            in_quote = true;
            quote_char = c;
            out.push(c);
        } else if c == '#' {
            break;
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_string_array(s: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_quote = false;
    let mut quote_char = '"';
    let mut current = String::new();

    for c in s.chars() {
        if in_quote {
            if c == quote_char {
                in_quote = false;
                items.push(std::mem::take(&mut current));
            } else {
                current.push(c);
            }
        } else if c == '"' || c == '\'' {
            in_quote = true;
            quote_char = c;
        }
    }
    items
}

fn find_config_for_tool(tool_name: &str, root: &Path) -> Option<ParsedToolsConfig> {
    let ws_root = WorkspaceResolver::find_root(Some(root)).unwrap_or_else(|_| root.to_path_buf());
    let mut candidates = vec![
        root.join("tools.toml"),
        root.join("config/tools.toml"),
        root.join("plugins/agy-suite/tools.toml"),
        ws_root.join("tools.toml"),
        ws_root.join("config/tools.toml"),
        ws_root.join("plugins/agy-suite/tools.toml"),
    ];

    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".gemini/config/plugins/agy-suite/tools.toml"));
    }

    for path in candidates {
        if path.is_file()
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let parsed = ParsedToolsConfig::from_str(&content);
            if parsed.tools.contains_key(tool_name) {
                return Some(parsed);
            }
        }
    }

    None
}

/// Checks whether a tool is declared as a mutating tool in declarative configuration.
pub(crate) fn is_mutating_tool(tool_name: &str, root: &Path) -> bool {
    find_config_for_tool(tool_name, root)
        .is_some_and(|cfg| cfg.is_tool_attribute(tool_name, "mutating"))
}

/// Inspects tools.toml (if present) for `require_permission_gate = true`.
pub(crate) fn is_tool_gated_in_config(tool_name: &str, root: &Path) -> bool {
    find_config_for_tool(tool_name, root)
        .is_some_and(|cfg| cfg.is_tool_attribute(tool_name, "require_permission_gate"))
}

/// Evaluates whether a tool requires a specific gate before execution.
#[must_use]
pub(crate) fn tool_requires_gate(tool_name: &str, root: &Path, gate_name: &str) -> bool {
    find_config_for_tool(tool_name, root)
        .is_some_and(|cfg| cfg.tool_requires_gate(tool_name, gate_name))
}

/// Evaluates whether a tool failure tracks strikes against the circuit breaker ledger.
#[must_use]
#[allow(dead_code, reason = "Declarative query utility for circuit breaker policy")]
pub(crate) fn tool_tracks_circuit_breaker(tool_name: &str, root: &Path) -> bool {
    find_config_for_tool(tool_name, root)
        .is_some_and(|cfg| cfg.is_tool_attribute(tool_name, "tracks_circuit_breaker"))
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
        let temp_dir = std::env::temp_dir().join(format!("hook_cb_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        drop(std::fs::create_dir_all(&temp_dir));
        drop(std::fs::write(
            temp_dir.join("tools.toml"),
            "[tools.modify_code]\nmutating = true\n",
        ));

        let mut ledger = CircuitBreakerLedger::new("coder");
        drop(ledger.record_failure(&temp_dir, Some("coder"), Some("quick_check"), Some("E0502"), Some("borrow error")));
        drop(ledger.record_failure(&temp_dir, Some("coder"), Some("quick_check"), Some("E0502"), Some("borrow error")));
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
