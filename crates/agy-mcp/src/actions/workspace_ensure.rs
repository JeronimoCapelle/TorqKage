//! In-process Jujutsu subagent workspace provisioner for the MCP daemon.

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
            "Missing required parameter 'agent' for ensure_agent_workspace".to_owned()
        })?;

    if agent.is_empty()
        || agent.starts_with('-')
        || !agent
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "Invalid agent name '{agent}': must start with an alphanumeric or underscore character and contain only alphanumeric, dash, or underscore characters"
        ));
    }

    match JujutsuWorkspace::ensure_agent_workspace(workspace_root, agent) {
        Ok(path) => Ok((
            format!("Isolated workspace ready at: {}", path.display()),
            false,
        )),
        Err(e) => Err(format!(
            "Failed to provision isolated workspace for agent '{agent}': {e}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_result_states, reason = "Test error state")]
    fn test_workspace_ensure_missing_agent() {
        let args = serde_json::json!({});
        assert!(execute(Path::new("."), &args).is_err());
    }

    #[test]
    #[allow(clippy::assertions_on_result_states, reason = "Test error state")]
    fn test_workspace_ensure_invalid_agent_name() {
        let args = serde_json::json!({"agent": "../malicious/path"});
        assert!(execute(Path::new("."), &args).is_err());

        let args_dash = serde_json::json!({"agent": "--help"});
        assert!(
            execute(Path::new("."), &args_dash)
                .is_err_and(|e| e.contains("must start with an alphanumeric"))
        );
    }
}
