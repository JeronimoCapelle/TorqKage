//! # gatekeeper
//!
//! Modular quality gatekeeper and pre-flight validation rules for specialized agents.

use std::{path::Path, process::Command};

use crate::{
    limits::{DEFAULT_PREVIEW_LINES, DEFAULT_VCS_QUERY_TIMEOUT_SECS},
    project::ProjectConfig,
    supervisor::SubprocessSupervisor,
};

/// Validation check outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateCheckStatus {
    /// Mandatory check passed completely.
    Passed,
    /// Mandatory check failed; execution must be blocked.
    Failed(String),
    /// Advisory warning; execution may proceed with warning.
    Warning(String),
}

/// A modular validation rule.
pub trait GateCheck: Send + Sync {
    /// Human-readable identifier of the check.
    fn name(&self) -> &str;
    /// Executes the check against the given workspace root directory.
    fn execute(&self, workspace_root: &Path) -> GateCheckStatus;
}

/// A declarative gate check backed by a command defined in `project.toml`.
#[derive(Debug, Clone)]
pub struct ProjectGateCheck {
    name: String,
    command: Vec<String>,
    timeout_seconds: u64,
    failure_message: Option<String>,
}

impl ProjectGateCheck {
    /// Constructs a new `ProjectGateCheck`.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        command: Vec<String>,
        timeout_seconds: u64,
        failure_message: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            command,
            timeout_seconds,
            failure_message,
        }
    }
}

fn is_binary_in_path(binary: &str) -> bool {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(binary);
            if full.is_file() {
                return true;
            }
        }
    }
    false
}

impl GateCheck for ProjectGateCheck {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&self, workspace_root: &Path) -> GateCheckStatus {
        let Some(binary) = self.command.first() else {
            return GateCheckStatus::Passed;
        };
        if !binary.contains('/') && !is_binary_in_path(binary) {
            return GateCheckStatus::Failed(format!(
                "Quality gate '{}' binary '{binary}' is not installed on host ($PATH)",
                self.name
            ));
        }

        let args = self.command.get(1..).unwrap_or(&[]);

        let mut cmd = Command::new(binary);
        cmd.args(args);
        cmd.current_dir(workspace_root);

        match SubprocessSupervisor::run_supervised(cmd, self.timeout_seconds) {
            Ok(out) => {
                if out.status.success() {
                    GateCheckStatus::Passed
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let raw = format!("{stdout}\n{stderr}");
                    let truncated: Vec<&str> = raw
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .take(DEFAULT_PREVIEW_LINES)
                        .collect();
                    let diag = truncated.join("\n");
                    let default_msg = format!(
                        "Gate '{}' failed (`{}`).",
                        self.name,
                        self.command.join(" ")
                    );
                    let msg = self.failure_message.as_deref().unwrap_or(&default_msg);
                    GateCheckStatus::Failed(format!("{msg}\n\n[Diagnostic Output]:\n{diag}"))
                }
            },
            Err(e) => {
                GateCheckStatus::Failed(format!("Failed to execute gate '{}': {e}", self.name))
            },
        }
    }
}

/// Validates that no unresolved VCS merge conflict markers exist in the working copy.
#[derive(Debug, Clone, Copy, Default)]
pub struct VcsConflictCheck;

impl GateCheck for VcsConflictCheck {
    fn name(&self) -> &'static str {
        "VCS Conflict Marker Check"
    }

    fn execute(&self, workspace_root: &Path) -> GateCheckStatus {
        // 1. Jujutsu conflict check
        if workspace_root.join(".jj").is_dir() {
            let mut cmd = Command::new("jj");
            cmd.args([
                "log",
                "-r",
                "conflicts() & @",
                "--no-graph",
                "-T",
                "change_id",
            ]);
            cmd.current_dir(workspace_root);

            if let Ok(out) =
                SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
                && out.status.success()
            {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if !stdout.is_empty() {
                    return GateCheckStatus::Failed(format!(
                        "Unresolved Jujutsu conflict in active commit (@: {stdout}). Resolve conflicts before subagent launch."
                    ));
                }
            }
        }

        // 2. Git conflict check
        if workspace_root.join(".git").is_dir() {
            let mut cmd = Command::new("git");
            cmd.args(["diff", "--check"]);
            cmd.current_dir(workspace_root);

            if let Ok(out) =
                SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
            {
                let combined = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                if combined.contains("leftover conflict marker")
                    || combined.contains("conflict marker")
                {
                    return GateCheckStatus::Failed(
                        "Unresolved conflict markers detected in working tree (git diff --check)."
                            .to_owned(),
                    );
                }
            }
        }

        GateCheckStatus::Passed
    }
}

/// Validates that no uncommitted modifications exist in the working copy.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkspaceCleanCheck;

impl GateCheck for WorkspaceCleanCheck {
    fn name(&self) -> &'static str {
        "Workspace Clean Check"
    }

    fn execute(&self, workspace_root: &Path) -> GateCheckStatus {
        if crate::JujutsuWorkspace::is_jj_repo(workspace_root) {
            if !crate::JujutsuWorkspace::is_workspace_clean(workspace_root) {
                return GateCheckStatus::Failed(
                    "Working copy has uncommitted modifications (Jujutsu repository not clean)."
                        .to_owned(),
                );
            }
            return GateCheckStatus::Passed;
        }

        if workspace_root.join(".git").exists() {
            let mut cmd = Command::new("git");
            cmd.args(["status", "--porcelain"]);
            cmd.current_dir(workspace_root);
            if let Ok(out) =
                SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
                && out.status.success()
                && !out.stdout.is_empty()
            {
                return GateCheckStatus::Failed(
                    "Working copy has uncommitted modifications (Git status --porcelain not clean).".to_owned(),
                );
            }
        }

        GateCheckStatus::Passed
    }
}

/// Evaluates pre-flight admission checks specified by name.
///
/// Built-in checks:
/// - `"vcs_conflicts"` -> `VcsConflictCheck`
/// - `"workspace_clean"` -> `WorkspaceCleanCheck`
/// - Any quality gate configured dynamically in `project.toml [gates.<name>]`
///
/// Fallback behavior:
/// - If a gate is required by an agent manifest but not configured in `project.toml`,
///   admission fails with a descriptive error. No language-specific commands are assumed.
///
/// # Errors
/// Returns a list of blocking failure diagnostics if any mandatory check fails.
pub fn evaluate_pre_flight_checks(
    workspace_root: &Path,
    check_names: &[&str],
) -> Result<Vec<String>, Vec<String>> {
    let project = ProjectConfig::detect_or_load(workspace_root);
    let mut checks: Vec<Box<dyn GateCheck>> = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for name in check_names {
        match *name {
            "vcs_conflicts" => checks.push(Box::new(VcsConflictCheck)),
            "workspace_clean" => checks.push(Box::new(WorkspaceCleanCheck)),
            gate_name => {
                if let Some(gate) = project.gates.get(gate_name) {
                    checks.push(Box::new(ProjectGateCheck::new(
                        format!("Quality Gate: {gate_name}"),
                        gate.command.clone(),
                        gate.timeout_seconds,
                        gate.failure_message.clone(),
                    )));
                } else if project.gates.is_empty() {
                    warnings.push(format!(
                        "Advisory: Gate '{gate_name}' was requested, but workspace has no quality gates configured in project.toml."
                    ));
                } else {
                    errors.push(format!(
                        "Pre-launch check '{gate_name}' is required by agent manifest, but no gate '[gates.{gate_name}]' is defined in project.toml."
                    ));
                }
            },
        }
    }

    for check in checks {
        match check.execute(workspace_root) {
            GateCheckStatus::Passed => {},
            GateCheckStatus::Failed(err) => errors.push(err),
            GateCheckStatus::Warning(warn) => warnings.push(warn),
        }
    }

    if errors.is_empty() {
        Ok(warnings)
    } else {
        Err(errors)
    }
}

/// Discovers the path to an agent's Markdown specification file.
#[must_use]
#[allow(
    clippy::print_stderr,
    reason = "Advisory notice when agent manifest is missing"
)]
pub fn find_agent_manifest_path(
    workspace_root: &Path,
    agent_name: &str,
) -> Option<std::path::PathBuf> {
    let found = crate::frontmatter::find_agent_manifest(workspace_root, agent_name);
    if found.is_none() {
        eprintln!(
            "[agy-common] Advisory: Agent manifest for '{agent_name}' not found across workspace or ~/.gemini/config."
        );
    }
    found
}

/// Parses the list of `pre_launch_checks` from an agent's Markdown frontmatter.
#[must_use]
pub fn parse_pre_launch_checks(content: &str) -> Vec<String> {
    crate::frontmatter::AgentFrontmatter::extract_pre_launch_checks(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pre_launch_checks() {
        let sample = "\
---
name: sentinel
pre_launch_checks:
  - compilation
  - vcs_conflicts
---
# Prompt
";
        let checks = parse_pre_launch_checks(sample);
        assert_eq!(checks, vec!["compilation", "vcs_conflicts"]);

        let empty_sample = "\
---
name: sentinel
pre_launch_checks: []
---
";
        let empty_checks = parse_pre_launch_checks(empty_sample);
        assert_eq!(empty_checks, Vec::<String>::new());
    }

    #[test]
    fn test_gate_check_trait_dispatch() {
        struct MockPass;
        impl GateCheck for MockPass {
            fn name(&self) -> &'static str {
                "Mock Pass"
            }

            fn execute(&self, _: &Path) -> GateCheckStatus {
                GateCheckStatus::Passed
            }
        }

        struct MockFail;
        impl GateCheck for MockFail {
            fn name(&self) -> &'static str {
                "Mock Fail"
            }

            fn execute(&self, _: &Path) -> GateCheckStatus {
                GateCheckStatus::Failed("error".to_owned())
            }
        }

        let pass = MockPass;
        let fail = MockFail;
        assert_eq!(pass.execute(Path::new(".")), GateCheckStatus::Passed);
        assert_eq!(
            fail.execute(Path::new(".")),
            GateCheckStatus::Failed("error".to_owned())
        );
    }
}
