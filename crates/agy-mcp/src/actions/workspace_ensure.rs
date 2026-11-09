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

    // Evaluate mandatory pre-launch quality gates if declared by the target agent
    let checks = agy_common::find_agent_manifest_path(workspace_root, agent)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or_else(Vec::new, |content| {
            agy_common::parse_pre_launch_checks(&content)
        });

    if !checks.is_empty() {
        let check_refs: Vec<&str> = checks.iter().map(String::as_str).collect();
        if let Err(errors) =
            agy_common::gatekeeper::evaluate_pre_flight_checks(workspace_root, &check_refs)
        {
            return Err(format!(
                "🛑 Subagent Admission Denied for '{agent}': Mandatory pre-launch quality gates failed.\n\
                 Target agent requirements: [{}]\n\n\
                 {}",
                checks.join(", "),
                errors.join("\n\n")
            ));
        }
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
