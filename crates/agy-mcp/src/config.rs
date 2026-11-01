//! Declarative `tools.toml` parser and inheritance resolution engine.

use std::collections::BTreeMap;
use std::path::Path;
use serde::Deserialize;
use crate::models::{OutputCapture, ToolAction, ToolArg, ToolDefinition};

#[derive(Debug, Clone, Deserialize, Default)]
struct RawTool {
    inherit: Option<String>,
    description: Option<String>,
    action: Option<ToolAction>,
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
    error_fallback: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawConfigFile {
    #[serde(default)]
    base_tools: BTreeMap<String, RawTool>,
    #[serde(default)]
    tools: BTreeMap<String, RawTool>,
}

/// Parses and resolves all tools defined in a `tools.toml` content string.
///
/// # Errors
/// Returns an error if the TOML syntax is invalid.
pub(crate) fn parse_tools_str(toml_str: &str) -> Result<Vec<ToolDefinition>, toml::de::Error> {
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

    Ok(resolved_tools)
}

/// Loads and resolves all tools from a given `tools.toml` file path.
///
/// # Errors
/// Returns an error if reading the file or parsing TOML fails.
pub(crate) fn load_tools_from_path(path: &Path) -> Result<Vec<ToolDefinition>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|err| format!("Failed to read tools configuration at '{}': {err}", path.display()))?;

    parse_tools_str(&content)
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

            [tools.modify_test_code]
            inherit = "base_tools.code_modifier"
            description = "Restricted test modifier"
            allowed_directories = ["tests"]

            [tools.simple_check]
            description = "Runs cargo check"
            action = "static_command"
            command = ["cargo", "check"]
        "#;

        let tools = parse_tools_str(toml_sample).expect("parse failed");
        assert_eq!(tools.len(), 2);

        let test_tool = tools.iter().find(|t| t.name == "modify_test_code").expect("not found");
        assert_eq!(test_tool.description, "Restricted test modifier");
        assert_eq!(test_tool.action, ToolAction::MpatchEdit);
        assert!(test_tool.enforce_workspace_bounds);
        assert!(test_tool.ban_dangerous_files);
        assert_eq!(test_tool.allowed_directories, vec!["tests".to_owned()]);

        let check_tool = tools.iter().find(|t| t.name == "simple_check").expect("not found");
        assert_eq!(check_tool.action, ToolAction::StaticCommand);
        assert_eq!(check_tool.command, vec!["cargo".to_owned(), "check".to_owned()]);
    }
}
