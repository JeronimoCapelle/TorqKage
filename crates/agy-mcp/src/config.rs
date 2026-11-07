//! Declarative `tools.toml` parser and inheritance resolution engine.

use std::path::Path;

use agy_common::tools::{RawToolsConfig, ToolDefinition, resolve_tools};

/// Fully loaded and resolved tools configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct LoadedConfiguration {
    pub(crate) tools: Vec<ToolDefinition>,
}

/// Parses and resolves tools defined in a `tools.toml` string.
///
/// # Errors
/// Returns an error if the TOML syntax is invalid.
pub(crate) fn parse_full_config_str(
    toml_str: &str,
) -> Result<LoadedConfiguration, toml::de::Error> {
    let raw: RawToolsConfig = toml::from_str(toml_str)?;
    let resolved_tools = resolve_tools(&raw);

    Ok(LoadedConfiguration {
        tools: resolved_tools,
    })
}

/// Loads and resolves tools from a given `tools.toml` file path.
///
/// # Errors
/// Returns an error if reading the file or parsing TOML fails.
pub(crate) fn load_full_config_from_path(path: &Path) -> Result<LoadedConfiguration, String> {
    let content = std::fs::read_to_string(path).map_err(|err| {
        format!(
            "Failed to read tools configuration at '{}': {err}",
            path.display()
        )
    })?;

    parse_full_config_str(&content).map_err(|err| {
        format!(
            "Failed to parse tools configuration at '{}': {err}",
            path.display()
        )
    })
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
    use crate::models::ToolAction;

    #[test]
    fn test_inheritance_and_override() {
        let toml_sample = r#"
            [base_tools.modifier_engine]
            description = "Base modifier engine"
            action = "mpatch_edit"
            enforce_workspace_bounds = true
            ban_dangerous_files = true

            [tools.modify_code]
            inherit = "base_tools.modifier_engine"
            description = "Restricted test modifier"
            allowed_directories = ["tests"]

            [tools.simple_check]
            description = "Runs static check"
            action = "static_command"
            command = ["echo", "check"]
        "#;

        let loaded = parse_full_config_str(toml_sample).expect("parse failed");
        let tools = loaded.tools;
        assert_eq!(tools.len(), 2);

        let test_tool = tools
            .iter()
            .find(|t| t.name == "modify_code")
            .expect("not found");
        assert_eq!(test_tool.description, "Restricted test modifier");
        assert_eq!(test_tool.action, ToolAction::MpatchEdit);
        assert!(test_tool.enforce_workspace_bounds());
        assert!(test_tool.ban_dangerous_files());
        assert_eq!(test_tool.allowed_directories, vec!["tests".to_owned()]);

        let check_tool = tools
            .iter()
            .find(|t| t.name == "simple_check")
            .expect("not found");
        assert_eq!(check_tool.action, ToolAction::StaticCommand);
        assert_eq!(
            check_tool.command,
            vec!["echo".to_owned(), "check".to_owned()]
        );
    }
}
