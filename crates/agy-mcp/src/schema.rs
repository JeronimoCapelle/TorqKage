//! Dynamic JSON Schema generator converting declarative tools into MCP tool specifications.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{ToolAction, ToolDefinition};

/// An individual Model Context Protocol (MCP) tool description emitted during `tools/list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpTool {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) input_schema: serde_json::Value,
}

impl McpTool {
    /// Builds an `McpTool` specification from a resolved `ToolDefinition`.
    #[must_use]
    pub(crate) fn from_definition(def: &ToolDefinition) -> Self {
        let input_schema = build_input_schema(def);

        Self {
            name: def.name.clone(),
            description: def.description.clone(),
            input_schema,
        }
    }
}

fn build_input_schema(def: &ToolDefinition) -> serde_json::Value {
    match &def.action {
        ToolAction::FileRead => json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read (relative to workspace)"
                },
                "start_line": {
                    "type": "integer",
                    "description": "1-based starting line number (optional)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "1-based ending line number (optional)"
                }
            },
            "required": ["path"]
        }),

        ToolAction::FileWrite => json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Target destination file path within allowed directories"
                },
                "content": {
                    "type": "string",
                    "description": "Full file content to write"
                }
            },
            "required": ["path", "content"]
        }),

        ToolAction::MpatchEdit => json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Target file path within allowed directories"
                },
                "patch": {
                    "type": "string",
                    "description": "Aider search/replace block or unified diff to apply via context-aware fuzzy mpatch"
                }
            },
            "required": ["path", "patch"]
        }),

        ToolAction::FileSearch => json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Text pattern or regex to search across repository files"
                },
                "path": {
                    "type": "string",
                    "description": "Optional specific directory or file path to search (defaults to all configured documentation paths)"
                },
                "is_regex": {
                    "type": "boolean",
                    "description": "Whether to evaluate pattern as a regular expression (defaults to true if pattern is valid regex)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Whether the search is case-sensitive (defaults to false)"
                }
            },
            "required": ["pattern"]
        }),

        ToolAction::CliManual => json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the CLI binary or command to look up manual, recipes, or usage for (e.g. 'git', 'make', 'pytest', 'valgrind')"
                },
                "query": {
                    "type": "string",
                    "description": "Optional specific topic or task to search from navi cheatsheets (e.g. 'rebase', 'cache', 'filter')"
                }
            }
        }),

        ToolAction::ParameterizedCommand
        | ToolAction::WorkspaceEnsure
        | ToolAction::WorkspaceInspect
        | ToolAction::WorkspaceMerge => build_parameterized_schema(&def.args),

        ToolAction::StaticCommand
        | ToolAction::CargoJsonDiagnostics
        | ToolAction::ProjectState
        | ToolAction::Unknown(_) => {
            json!({
                "type": "object",
                "properties": {}
            })
        },
    }
}

fn build_parameterized_schema(args: &[crate::models::ToolArg]) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for arg in args {
        let mut prop = serde_json::Map::new();
        if arg.switch {
            prop.insert("type".to_owned(), json!("boolean"));
            if let Some(ref def) = arg.default {
                let is_truthy = def == "true" || def == "1";
                prop.insert("default".to_owned(), json!(is_truthy));
            }
        } else {
            prop.insert("type".to_owned(), json!("string"));
            if let Some(ref def) = arg.default {
                prop.insert("default".to_owned(), json!(def));
            }
        }
        if !arg.description.is_empty() {
            prop.insert("description".to_owned(), json!(arg.description));
        }
        properties.insert(arg.name.clone(), serde_json::Value::Object(prop));

        if arg.required {
            required.push(arg.name.clone());
        }
    }

    if required.is_empty() {
        json!({
            "type": "object",
            "properties": properties
        })
    } else {
        json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;
    use crate::models::ToolArg;

    #[test]
    fn test_file_read_schema() {
        let def = ToolDefinition {
            name: "read_project_file".to_owned(),
            description: "Read file".to_owned(),
            action: ToolAction::FileRead,
            ..Default::default()
        };

        let mcp_tool = McpTool::from_definition(&def);
        assert_eq!(mcp_tool.name, "read_project_file");
        assert_eq!(mcp_tool.input_schema["required"], json!(["path"]));
    }

    #[test]
    fn test_parameterized_command_schema() {
        let def = ToolDefinition {
            name: "cargo_expand".to_owned(),
            description: "Expand macro".to_owned(),
            action: ToolAction::ParameterizedCommand,
            args: vec![ToolArg {
                name: "item_path".to_owned(),
                description: "Target item path".to_owned(),
                required: true,
                ..Default::default()
            }],
            ..Default::default()
        };

        let mcp_tool = McpTool::from_definition(&def);
        assert_eq!(mcp_tool.input_schema["required"], json!(["item_path"]));
        assert!(mcp_tool.input_schema["properties"]["item_path"].is_object());
    }

    #[test]
    fn test_file_search_schema() {
        let def = ToolDefinition {
            name: "query_project_spec".to_owned(),
            description: "Search docs".to_owned(),
            action: ToolAction::FileSearch,
            ..Default::default()
        };

        let mcp_tool = McpTool::from_definition(&def);
        assert_eq!(mcp_tool.input_schema["required"], json!(["pattern"]));
        assert!(mcp_tool.input_schema["properties"]["pattern"].is_object());
        assert!(mcp_tool.input_schema["properties"]["is_regex"].is_object());
        assert!(mcp_tool.input_schema["properties"]["case_sensitive"].is_object());
    }
}
