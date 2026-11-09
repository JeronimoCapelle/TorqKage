//! Subagent admission gate verifying declared `pre_launch_checks`.

use std::path::Path;

use crate::gatekeeper::{
    evaluate_pre_flight_checks, find_agent_manifest_path, parse_pre_launch_checks,
};

/// Result of evaluating a subagent admission pre-launch check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentAdmissionResult {
    /// Name of the target agent being admitted.
    pub target_agent: String,
    /// List of declared pre-launch quality gates.
    pub declared_checks: Vec<String>,
    /// Whether admission succeeded without any blocking errors.
    pub passed: bool,
    /// Non-fatal advisory notices emitted by quality gates.
    pub warnings: Vec<String>,
    /// Blocking quality gate failure diagnostics.
    pub errors: Vec<String>,
}

impl SubagentAdmissionResult {
    /// Formats a standardized blocking denial diagnostic message.
    #[must_use]
    pub fn format_denial_reason(&self) -> String {
        format!(
            "🛑 Subagent Admission Denied for '{}': Mandatory pre-launch quality gates failed.\n\
             Target agent requirements: [{}]\n\n\
             {}",
            self.target_agent,
            self.declared_checks.join(", "),
            self.errors.join("\n\n")
        )
    }
}

/// Inspects declared pre-launch gates for `target_agent` and executes all quality checks.
#[must_use]
pub fn inspect_subagent_admission(
    workspace_root: &Path,
    target_agent: &str,
) -> SubagentAdmissionResult {
    let checks = find_agent_manifest_path(workspace_root, target_agent)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or_else(Vec::new, |content| parse_pre_launch_checks(&content));

    if checks.is_empty() {
        return SubagentAdmissionResult {
            target_agent: target_agent.to_owned(),
            declared_checks: Vec::new(),
            passed: true,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
    }

    let check_refs: Vec<&str> = checks.iter().map(String::as_str).collect();
    match evaluate_pre_flight_checks(workspace_root, &check_refs) {
        Ok(warnings) => SubagentAdmissionResult {
            target_agent: target_agent.to_owned(),
            declared_checks: checks,
            passed: true,
            warnings,
            errors: Vec::new(),
        },
        Err(errors) => SubagentAdmissionResult {
            target_agent: target_agent.to_owned(),
            declared_checks: checks,
            passed: false,
            warnings: Vec::new(),
            errors,
        },
    }
}

/// Evaluates subagent admission quality gates, returning `Ok(warnings)` or a formatted denial error string.
///
/// # Errors
/// Returns an error message if any declared pre-launch quality gates fail.
pub fn check_subagent_admission(
    workspace_root: &Path,
    target_agent: &str,
) -> Result<Vec<String>, String> {
    let result = inspect_subagent_admission(workspace_root, target_agent);
    if result.passed {
        Ok(result.warnings)
    } else {
        Err(result.format_denial_reason())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::assert_is_empty,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_subagent_admission_nonexistent_agent() {
        let temp_dir = std::env::temp_dir();
        let res = inspect_subagent_admission(&temp_dir, "nonexistent-subagent");
        assert!(res.passed);
        assert!(res.declared_checks.is_empty());
        assert!(res.errors.is_empty());
    }

    #[test]
    fn test_subagent_admission_denial_formatting() {
        let result = SubagentAdmissionResult {
            target_agent: "strict-tester".to_owned(),
            declared_checks: vec!["compilation".to_owned(), "tests".to_owned()],
            passed: false,
            warnings: Vec::new(),
            errors: vec!["error[E0425]: cannot find value `foo`".to_owned()],
        };

        let msg = result.format_denial_reason();
        assert!(msg.contains("Subagent Admission Denied for 'strict-tester'"));
        assert!(msg.contains("compilation, tests"));
        assert!(msg.contains("cannot find value `foo`"));
    }
}
