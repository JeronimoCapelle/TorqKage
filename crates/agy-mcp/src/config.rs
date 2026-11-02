//! Declarative `tools.toml` parser and inheritance resolution engine.

use std::collections::BTreeMap;
use std::path::Path;
use serde::Deserialize;
use crate::models::{AgentGuard, OutputCapture, ToolAction, ToolArg, ToolDefinition};

#[derive(Debug, Clone, Deserialize, Default)]
struct RawTool {
    inherit: Option<String>,
    description: Option<String>,
    action: Option<ToolAction>,
    category: Option<String>,
    mutating: Option<bool>,
    tracks_circuit_breaker: Option<bool>,
    command: Option<Vec<String>>,
    args: Option<Vec<ToolArg>>,
    output_capture: Option<OutputCapture>,
    allowed_directories: Option<Vec<String>>,
    enforce_dir: Option<String>,
    enforce_file_target: Option<String>,
    enforce_workspace_bounds: Option<bool>,
    ban_dangerous_files: Option<bool>,
    sanitize_input: Option<String>,
    truncate_lines: Option<usize>,
    truncate_bytes: Option<usize>,
    emit_truncation_notice: Option<bool>,
    attach_rustc_explain: Option<bool>,
    timeout_seconds: Option<u64>,
    require_permission_gate: Option<bool>,
    enforce_sandbox: Option<bool>,
    sandbox: Option<bool>,
    require_git_clean: Option<bool>,
    require_workspace_clean: Option<bool>,
    require_clippy_clean: Option<bool>,
    require_tests_passed: Option<bool>,
    error_fallback: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawConfigFile {
    #[serde(default)]
    base_tools: BTreeMap<String, RawTool>,
    #[serde(default)]
    tools: BTreeMap<String, RawTool>,
    #[serde(default)]
    agent_guards: BTreeMap<String, AgentGuard>,
}

/// Fully loaded and resolved tools and agent guards configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct LoadedConfiguration {
    pub(crate) tools: Vec<ToolDefinition>,
    pub(crate) agent_guards: BTreeMap<String, AgentGuard>,
}

/// Parses and resolves both tools and agent guards defined in a `tools.toml` string.
///
/// # Errors
/// Returns an error if the TOML syntax is invalid.
pub(crate) fn parse_full_config_str(
    toml_str: &str,
) -> Result<LoadedConfiguration, toml::de::Error> {
    let raw: RawConfigFile = toml::from_str(toml_str)?;

    let mut resolved_tools = Vec::with_capacity(raw.tools.len());

    for (name, raw_tool) in raw.tools {
        let base = raw_tool.inherit.as_deref().and_then(|inherit_key| {
            let key = inherit_key.strip_prefix("base_tools.").unwrap_or(inherit_key);
            raw.base_tools.get(key)
        });

        let tool = merge_tool(name, base, &raw_tool);
        resolved_tools.push(tool);
    }

    Ok(LoadedConfiguration {
        tools: resolved_tools,
        agent_guards: raw.agent_guards,
    })
}

/// Loads and resolves tools and agent guards from a given `tools.toml` file path.
///
/// # Errors
/// Returns an error if reading the file or parsing TOML fails.
pub(crate) fn load_full_config_from_path(path: &Path) -> Result<LoadedConfiguration, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|err| format!("Failed to read tools configuration at '{}': {err}", path.display()))?;

    parse_full_config_str(&content)
        .map_err(|err| format!("Failed to parse tools configuration at '{}': {err}", path.display()))
}

fn merge_tool(name: String, base: Option<&RawTool>, raw: &RawTool) -> ToolDefinition {
    ToolDefinition {
        name,
        description: raw
            .description
            .clone()
            .or_else(|| base?.description.clone())
            .unwrap_or_default(),
        action: raw
            .action
            .clone()
            .or_else(|| base?.action.clone())
            .unwrap_or_default(),
        category: raw
            .category
            .clone()
            .or_else(|| base?.category.clone()),
        mutating: raw
            .mutating
            .or_else(|| base?.mutating)
            .unwrap_or(false),
        tracks_circuit_breaker: raw
            .tracks_circuit_breaker
            .or_else(|| base?.tracks_circuit_breaker)
            .unwrap_or(false),
        command: raw
            .command
            .clone()
            .or_else(|| base?.command.clone())
            .unwrap_or_default(),
        args: raw
            .args
            .clone()
            .or_else(|| base?.args.clone())
            .unwrap_or_default(),
        output_capture: raw
            .output_capture
            .or_else(|| base?.output_capture)
            .unwrap_or_default(),
        allowed_directories: raw
            .allowed_directories
            .clone()
            .or_else(|| base?.allowed_directories.clone())
            .unwrap_or_default(),
        enforce_dir: raw
            .enforce_dir
            .clone()
            .or_else(|| base?.enforce_dir.clone()),
        enforce_file_target: raw
            .enforce_file_target
            .clone()
            .or_else(|| base?.enforce_file_target.clone()),
        enforce_workspace_bounds: raw
            .enforce_workspace_bounds
            .or_else(|| base?.enforce_workspace_bounds)
            .unwrap_or(false),
        ban_dangerous_files: raw
            .ban_dangerous_files
            .or_else(|| base?.ban_dangerous_files)
            .unwrap_or(false),
        sanitize_input: raw
            .sanitize_input
            .clone()
            .or_else(|| base?.sanitize_input.clone()),
        truncate_lines: raw.truncate_lines.or_else(|| base?.truncate_lines),
        truncate_bytes: raw.truncate_bytes.or_else(|| base?.truncate_bytes),
        emit_truncation_notice: raw
            .emit_truncation_notice
            .or_else(|| base?.emit_truncation_notice)
            .unwrap_or(false),
        attach_rustc_explain: raw
            .attach_rustc_explain
            .or_else(|| base?.attach_rustc_explain)
            .unwrap_or(false),
        timeout_seconds: raw
            .timeout_seconds
            .or_else(|| base?.timeout_seconds)
            .unwrap_or(30),
        require_permission_gate: raw
            .require_permission_gate
            .or_else(|| base?.require_permission_gate)
            .unwrap_or(false),
        enforce_sandbox: raw
            .enforce_sandbox
            .or(raw.sandbox)
            .or_else(|| base?.enforce_sandbox.or(base?.sandbox))
            .unwrap_or(false),
        require_git_clean: raw
            .require_workspace_clean
            .or(raw.require_git_clean)
            .or_else(|| base?.require_workspace_clean.or(base?.require_git_clean))
            .unwrap_or(false),
        require_clippy_clean: raw
            .require_clippy_clean
            .or_else(|| base?.require_clippy_clean)
            .unwrap_or(false),
        require_tests_passed: raw
            .require_tests_passed
            .or_else(|| base?.require_tests_passed)
            .unwrap_or(false),
        error_fallback: raw
            .error_fallback
            .clone()
            .or_else(|| base?.error_fallback.clone()),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_inheritance_and_override() {
        let toml_sample = r#"
            [base_tools.code_modifier]
            description = "Base modifier engine"
            action = "mpatch_edit"
            enforce_workspace_bounds = true
            ban_dangerous_files = true

            [tools.modify_code]
            inherit = "base_tools.code_modifier"
            description = "Restricted test modifier"
            allowed_directories = ["tests"]

            [tools.simple_check]
            description = "Runs cargo check"
            action = "static_command"
            command = ["cargo", "check"]
        "#;

        let loaded = parse_full_config_str(toml_sample).expect("parse failed");
        let tools = loaded.tools;
        assert_eq!(tools.len(), 2);

        let test_tool = tools.iter().find(|t| t.name == "modify_code").expect("not found");
        assert_eq!(test_tool.description, "Restricted test modifier");
        assert_eq!(test_tool.action, ToolAction::MpatchEdit);
        assert!(test_tool.enforce_workspace_bounds);
        assert!(test_tool.ban_dangerous_files);
        assert_eq!(test_tool.allowed_directories, vec!["tests".to_owned()]);

        let check_tool = tools.iter().find(|t| t.name == "simple_check").expect("not found");
        assert_eq!(check_tool.action, ToolAction::StaticCommand);
        assert_eq!(check_tool.command, vec!["cargo".to_owned(), "check".to_owned()]);
    }

    #[test]
    fn test_parse_agent_guards() {
        let toml_sample = r#"
            [tools.write_file]
            description = "Writes a file"
            action = "file_write"

            [agent_guards.coder]
            allowed_dirs = ["src"]
            disallowed_dirs = ["tests", "benches"]

            [agent_guards.planner]
            allowed_dirs = ["docs"]
            allowed_files = ["ARCHITECTURE.md"]
        "#;

        let loaded = parse_full_config_str(toml_sample).expect("parse failed");
        assert_eq!(loaded.tools.len(), 1);
        assert_eq!(loaded.agent_guards.len(), 2);

        let coder_guard = loaded.agent_guards.get("coder").expect("coder guard missing");
        assert_eq!(coder_guard.allowed_dirs, vec!["src".to_owned()]);
        assert_eq!(coder_guard.disallowed_dirs, vec!["tests".to_owned(), "benches".to_owned()]);

        let planner_guard = loaded.agent_guards.get("planner").expect("planner guard missing");
        assert_eq!(planner_guard.allowed_dirs, vec!["docs".to_owned()]);
        assert_eq!(planner_guard.allowed_files, vec!["ARCHITECTURE.md".to_owned()]);
    }
}
