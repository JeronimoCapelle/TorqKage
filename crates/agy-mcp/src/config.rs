//! Declarative `tools.toml` parser and inheritance resolution engine.

use std::{collections::BTreeMap, path::Path};

use agy_common::tools::{RawToolsConfig, ToolDefinition, resolve_tools};
use serde::Deserialize;

use crate::models::AgentGuard;

#[derive(Debug, Clone, Deserialize, Default)]
struct RawConfigFile {
    #[serde(flatten)]
    tools_config: RawToolsConfig,
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
    let resolved_tools = resolve_tools(&raw.tools_config);

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
            vec!["cargo".to_owned(), "check".to_owned()]
        );
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

        let coder_guard = loaded
            .agent_guards
            .get("coder")
            .expect("coder guard missing");
        assert_eq!(coder_guard.allowed_dirs, vec!["src".to_owned()]);
        assert_eq!(
            coder_guard.disallowed_dirs,
            vec!["tests".to_owned(), "benches".to_owned()]
        );

        let planner_guard = loaded
            .agent_guards
            .get("planner")
            .expect("planner guard missing");
        assert_eq!(planner_guard.allowed_dirs, vec!["docs".to_owned()]);
        assert_eq!(
            planner_guard.allowed_files,
            vec!["ARCHITECTURE.md".to_owned()]
        );
    }
}
