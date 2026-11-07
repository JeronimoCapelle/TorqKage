//! In-process Jujutsu subagent workspace inspection action for the MCP daemon.

use std::path::Path;

use agy_common::JujutsuWorkspace;

pub(crate) fn execute(
    workspace_root: &Path,
    arguments: &serde_json::Value,
) -> Result<(String, bool), String> {
    let agent = arguments
        .get("agent")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            "Missing required parameter 'agent' for inspect_agent_workspace".to_owned()
        })?;

    if !agent
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "Invalid agent name '{agent}': must contain only alphanumeric, dash, or underscore characters"
        ));
    }

    match JujutsuWorkspace::inspect_agent_workspace(workspace_root, agent) {
        Ok(output) => Ok((output, false)),
        Err(e) => Err(format!(
            "Failed to inspect isolated workspace for agent '{agent}': {e}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_result_states, reason = "Test error state")]
    fn test_workspace_inspect_missing_agent() {
        let args = serde_json::json!({});
        assert!(execute(Path::new("."), &args).is_err());
    }

    #[test]
    #[allow(clippy::assertions_on_result_states, reason = "Test error state")]
    fn test_workspace_inspect_invalid_agent_name() {
        let args = serde_json::json!({"agent": "../malicious/path"});
        assert!(execute(Path::new("."), &args).is_err());
    }
}
