//! # launcher
//!
//! Interactive process launcher and pre-flight validation pipeline for dispatching `agy` sessions from `agy-ctrl`.

use std::io::{self, stdout};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use crate::model::AgentManifest;

/// Result of evaluating pre-launch checks prior to starting an `agy` session.
#[derive(Debug, Clone)]
pub(crate) struct PreLaunchReport {
    pub(crate) agent_name: String,
    pub(crate) errors: Vec<String>,
    pub(crate) warnings: Vec<String>,
}

impl PreLaunchReport {
    /// Constructs a new empty report for the given agent.
    #[must_use]
    pub(crate) fn new(agent_name: &str) -> Self {
        Self {
            agent_name: agent_name.to_owned(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Returns `true` if all mandatory pre-launch checks succeeded.
    #[must_use]
    pub(crate) const fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Formats errors and warnings into a readable diagnostic string.
    #[must_use]
    pub(crate) fn format_diagnostics(&self) -> String {
        let mut msg = format!("Pre-launch checks for agent '{}':\n", self.agent_name);
        if !self.errors.is_empty() {
            msg.push_str("\n❌ Blocking Errors:\n");
            for err in &self.errors {
                msg.push_str(&format!("  • {err}\n"));
            }
        }
        if !self.warnings.is_empty() {
            msg.push_str("\n⚠ Warnings:\n");
            for warn in &self.warnings {
                msg.push_str(&format!("  • {warn}\n"));
            }
        }
        msg
    }
}

/// Executes all mandatory and advisory pre-launch checks before booting an `agy` session.
///
/// This provides a modular, extensible hook for gating agent execution.
pub(crate) fn run_pre_launch_checks(workspace_root: &Path, agent: &AgentManifest) -> PreLaunchReport {
    let mut report = PreLaunchReport::new(&agent.name);

    // Check 1: Verify agy executable is discoverable in system PATH
    if which::which("agy").is_err() {
        report.errors.push(
            "The 'agy' CLI executable could not be found in system PATH. Ensure Antigravity is installed.".to_owned(),
        );
    }

    // Check 2: Verify the agent's manifest file exists on disk
    if !agent.file_path.is_file() {
        report.errors.push(format!(
            "Agent manifest does not exist at '{}'. Please save or restore configurations.",
            agent.file_path.display()
        ));
    }

    // Check 3: Check for uncommitted/unsaved changes in the agy-ctrl editor buffer
    if agent.modified {
        report.errors.push(format!(
            "Agent '{}' has unsaved modifications in agy-ctrl. Press [F2] to Save & Validate before launching.",
            agent.name
        ));
    }

    // Check 4: Plugin and tools configuration integrity
    let tools_toml = workspace_root.join("tools.toml");
    let plugin_tools = workspace_root.join("plugins/agy-suite/tools.toml");
    let has_home_tools = std::env::var("HOME").is_ok_and(|home| {
        Path::new(&home).join(".gemini/config/plugins/agy-suite/tools.toml").is_file()
    });
    if !tools_toml.is_file() && !plugin_tools.is_file() && !has_home_tools {
        report.warnings.push("No tools.toml configuration found in workspace or plugin install.".to_owned());
    }

    // Check 5: Air-gapped / security notice
    if agent.mcp_tools.is_empty() && agent.native_tools.is_empty() {
        report.warnings.push(
            "Agent has no MCP or native tools assigned. It will run in completely air-gapped baseline mode.".to_owned(),
        );
    }

    // Check 6: Pre-flight admission checks declared in agent frontmatter
    if !agent.pre_launch_checks.is_empty() {
        let check_refs: Vec<&str> = agent.pre_launch_checks.iter().map(String::as_str).collect();
        match agy_common::evaluate_pre_flight_checks(workspace_root, &check_refs) {
            Ok(warns) => {
                report.warnings.extend(warns);
            }
            Err(errs) => {
                report.errors.extend(errs);
            }
        }
    }

    report
}

/// Suspends Ratatui, runs pre-launch checks, and spawns an interactive `agy` CLI session.
///
/// If `resume_conversation` is specified, resumes that specific session using `agy --conversation <id>`.
/// Otherwise, boots a new session with `agy --agent <name>`.
///
/// Once the user exits the `agy` session, raw mode and alternate screen are re-established,
/// returning the user seamlessly back to the `agy-ctrl` deck.
///
/// # Errors
/// Returns an error string if pre-launch checks fail or if `agy` cannot be executed.
pub(crate) fn launch_agy_interactive(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    workspace_root: &Path,
    agent: &AgentManifest,
    sandbox: bool,
    isolated_workspace: bool,
    resume_conversation: Option<&str>,
) -> Result<ExitStatus, String> {
    // 1. Run pre-launch checks
    let report = run_pre_launch_checks(workspace_root, agent);
    if !report.is_ok() {
        return Err(report.format_diagnostics());
    }

    // 2. Suspend Ratatui & restore terminal to normal canonical mode
    drop(disable_raw_mode());
    drop(execute!(stdout(), LeaveAlternateScreen));
    drop(terminal.show_cursor());

    let (effective_workspace, was_isolated) = if isolated_workspace && agy_common::JujutsuWorkspace::is_jj_repo(workspace_root) {
        match agy_common::JujutsuWorkspace::ensure_agent_workspace(workspace_root, &agent.name) {
            Ok(ws_path) => (ws_path, true),
            Err(e) => {
                eprintln!("⚠ [agy-ctrl] Failed to initialize isolated workspace: {e}. Falling back to root.");
                (workspace_root.to_path_buf(), false)
            }
        }
    } else {
        (workspace_root.to_path_buf(), false)
    };

    // Print friendly launcher header
    println!("\n================================================================================");
    if let Some(conv_id) = resume_conversation {
        println!("🚀 [agy-ctrl] RESUMING previous agy session for agent: {}", agent.name);
        println!("   Conversation ID: {conv_id}");
    } else {
        println!("🚀 [agy-ctrl] Launching fresh agy session with agent: {}", agent.name);
    }
    println!("   Effort: {} | Permissions: {} | Policy: {}", agent.effort, agent.permission_mode, agent.command_execution_policy);
    println!("   Sandbox: {}", if sandbox { "ENABLED (Bubblewrap isolation)" } else { "DISABLED (direct host)" });
    if was_isolated {
        println!("   Workspace: {} (🛡️ Jujutsu Isolated Workspace)", effective_workspace.display());
        println!("   (Main working copy @ and editor buffers are fully protected)");
    } else {
        println!("   Workspace: {}", workspace_root.display());
    }
    println!("   (Exiting the agy session will return you directly to agy-ctrl)");
    println!("================================================================================\n");

    // 3. Spawn agy with inherited stdio
    let mut cmd = Command::new("agy");
    if sandbox {
        cmd.arg("--sandbox");
    }
    if let Some(conv_id) = resume_conversation {
        cmd.arg("--conversation").arg(conv_id);
    } else {
        cmd.arg("--agent").arg(&agent.name);
    }
    cmd.current_dir(&effective_workspace);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status_res = cmd.status();

    if was_isolated {
        println!("\n--------------------------------------------------------------------------------");
        println!("🏁 [agy-ctrl] Session ended in isolated workspace: .workspaces/{}", agent.name);
        println!("   Changes are committed to workspace '{}@'.", agent.name);
        println!("   Use 'jj workspace list' or 'jj diff -R .workspaces/{}' to inspect.", agent.name);
        println!("--------------------------------------------------------------------------------");
    }

    // 4. Resume Ratatui & crossterm alternate screen regardless of status
    drop(enable_raw_mode());
    let mut stdout_handle = stdout();
    drop(execute!(stdout_handle, EnterAlternateScreen));
    drop(terminal.clear());
    drop(terminal.hide_cursor());

    status_res.map_err(|e| format!("Failed to execute 'agy': {e}"))
}

/// Headless launcher: runs pre-launch checks and spawns `agy` directly from CLI.
///
/// # Errors
/// Returns an error string if pre-launch checks fail or if `agy` cannot be spawned.
pub(crate) fn launch_agy_headless(
    workspace_root: &Path,
    agent: &AgentManifest,
    sandbox: bool,
    isolated_workspace: bool,
    resume_conversation: Option<&str>,
) -> Result<ExitStatus, String> {
    let report = run_pre_launch_checks(workspace_root, agent);
    if !report.is_ok() {
        return Err(report.format_diagnostics());
    }

    let (effective_workspace, was_isolated) = if isolated_workspace && agy_common::JujutsuWorkspace::is_jj_repo(workspace_root) {
        match agy_common::JujutsuWorkspace::ensure_agent_workspace(workspace_root, &agent.name) {
            Ok(ws_path) => (ws_path, true),
            Err(e) => {
                eprintln!("⚠ [agy-ctrl] Failed to initialize isolated workspace: {e}. Falling back to root.");
                (workspace_root.to_path_buf(), false)
            }
        }
    } else {
        (workspace_root.to_path_buf(), false)
    };

    if let Some(conv_id) = resume_conversation {
        println!(
            "🚀 [agy-ctrl] Resuming agy session {conv_id} with agent: {} (Sandbox: {}, Isolated: {})",
            agent.name,
            if sandbox { "ENABLED" } else { "DISABLED" },
            if was_isolated { "YES (.workspaces)" } else { "NO" }
        );
    } else {
        println!(
            "🚀 [agy-ctrl] Launching agy session with agent: {} (Sandbox: {}, Isolated: {})",
            agent.name,
            if sandbox { "ENABLED" } else { "DISABLED" },
            if was_isolated { "YES (.workspaces)" } else { "NO" }
        );
    }

    let mut cmd = Command::new("agy");
    if sandbox {
        cmd.arg("--sandbox");
    }
    if let Some(conv_id) = resume_conversation {
        cmd.arg("--conversation").arg(conv_id);
    } else {
        cmd.arg("--agent").arg(&agent.name);
    }
    cmd.current_dir(&effective_workspace);
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let res = cmd.status().map_err(|e| format!("Failed to spawn 'agy': {e}"));

    if was_isolated {
        println!("\n🏁 [agy-ctrl] Session ended in isolated workspace: .workspaces/{}", agent.name);
        println!("   Changes are committed to workspace '{}@'.", agent.name);
    }

    res
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
    use std::path::PathBuf;

    #[test]
    fn test_pre_launch_report_diagnostics() {
        let mut report = PreLaunchReport::new("test-agent");
        assert!(report.is_ok());

        report.errors.push("Failed critical check".to_owned());
        assert!(!report.is_ok());

        let diag = report.format_diagnostics();
        assert!(diag.contains("test-agent"));
        assert!(diag.contains("Failed critical check"));
    }

    #[test]
    fn test_run_pre_launch_checks_unsaved_changes() {
        let manifest = AgentManifest {
            file_path: PathBuf::from("nonexistent.md"),
            name: "dirty-agent".to_owned(),
            description: "test".to_owned(),
            effort: "low".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            command_execution_policy: "off".to_owned(),
            main_agent: false,
            subagent: true,
            inherit_mcp: false,
            allowed_subagents: Vec::new(),
            pre_launch_checks: Vec::new(),
            native_tools: Vec::new(),
            mcp_tools: Vec::new(),
            directives: String::new(),
            modified: true,
        };

        let report = run_pre_launch_checks(Path::new("."), &manifest);
        assert!(!report.is_ok());
        assert!(report.errors.iter().any(|e| e.contains("unsaved modifications")));
    }

    #[test]
    fn test_run_pre_launch_checks_declarative_gatekeeper() {
        let manifest = AgentManifest {
            file_path: PathBuf::from("nonexistent.md"),
            name: "declarative-sentinel".to_owned(),
            description: "test sentinel".to_owned(),
            effort: "high".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            command_execution_policy: "prompt".to_owned(),
            main_agent: false,
            subagent: true,
            inherit_mcp: false,
            allowed_subagents: Vec::new(),
            pre_launch_checks: vec![
                "cargo_compilation".to_owned(),
                "test_compilation".to_owned(),
                "vcs_conflicts".to_owned(),
            ],
            native_tools: Vec::new(),
            mcp_tools: Vec::new(),
            directives: String::new(),
            modified: false,
        };

        // When run against an empty temporary directory with no Cargo.toml, gatekeeper passes cleanly
        let temp_dir = std::env::temp_dir();
        let report = run_pre_launch_checks(&temp_dir, &manifest);
        // The only errors should be missing manifest file and possibly agy CLI if PATH is weird,
        // but no cargo check errors from gatekeeper
        assert!(!report.errors.iter().any(|e| e.contains("requires 100% clean compilation")));
    }

    #[test]
    fn test_launch_headless_pre_check_failure() {
        let manifest = AgentManifest {
            file_path: PathBuf::from("nonexistent.md"),
            name: "broken-agent".to_owned(),
            description: "test".to_owned(),
            effort: "low".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            command_execution_policy: "off".to_owned(),
            main_agent: false,
            subagent: true,
            inherit_mcp: false,
            allowed_subagents: Vec::new(),
            pre_launch_checks: Vec::new(),
            native_tools: Vec::new(),
            mcp_tools: Vec::new(),
            directives: String::new(),
            modified: true, // triggers pre-check error
        };

        let res = launch_agy_headless(Path::new("."), &manifest, false, true, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("unsaved modifications"));
    }
}
