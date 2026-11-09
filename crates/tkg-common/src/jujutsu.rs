//! Jujutsu (`jj`) workspace and isolation primitives.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    error::{Result, TkgError},
    limits::DEFAULT_VCS_QUERY_TIMEOUT_SECS,
    supervisor::SubprocessSupervisor,
};

/// Primitives for managing isolated Jujutsu workspaces.
#[derive(Debug, Clone, Copy)]
pub struct JujutsuWorkspace;

impl JujutsuWorkspace {
    /// Checks whether the target directory or its ancestors is a Jujutsu repository.
    #[must_use]
    pub fn is_jj_repo(workspace_root: &Path) -> bool {
        if workspace_root.join(".jj").exists() {
            return true;
        }
        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "root"]);
        cmd.current_dir(workspace_root);
        SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
            .is_ok_and(|out| out.status.success())
    }

    /// Returns the active workspace name (e.g., `"default"`, `"subagent_a"`) for `workspace_root`.
    #[must_use]
    pub fn current_workspace_name(workspace_root: &Path) -> Option<String> {
        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "workspace", "root"]);
        cmd.current_dir(workspace_root);
        let output =
            SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS).ok()?;
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let p = Path::new(&path_str);
            return p
                .file_name()
                .and_then(|n| n.to_str())
                .map(ToOwned::to_owned);
        }
        None
    }

    /// Checks if a workspace with the given name already exists in the repo.
    #[must_use]
    pub fn workspace_exists(workspace_root: &Path, name: &str) -> bool {
        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "workspace", "list"]);
        cmd.current_dir(workspace_root);
        let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);
        if let Ok(out) = output
            && out.status.success()
        {
            let list_str = String::from_utf8_lossy(&out.stdout);
            for line in list_str.lines() {
                if let Some(first) = line.split(':').next() {
                    let ws_id = first.split_whitespace().next().unwrap_or("").trim();
                    if ws_id == name {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Checks if an isolated workspace has zero uncommitted changes.
    #[must_use]
    #[allow(
        clippy::print_stderr,
        reason = "Advisory notice when jj binary is absent"
    )]
    pub fn is_workspace_clean(ws_path: &Path) -> bool {
        if !is_jj_in_path() {
            eprintln!(
                "[tkg-common] Advisory: 'jj' (Jujutsu) CLI not found in PATH; cannot check workspace clean state."
            );
            return false;
        }

        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "diff", "--summary"]);
        cmd.current_dir(ws_path);
        let Ok(out) = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
        else {
            return false;
        };

        if !out.status.success() {
            return false;
        }

        let project = crate::project::ProjectConfig::detect_or_load(ws_path);
        let stdout = String::from_utf8_lossy(&out.stdout);
        !stdout.lines().any(|l| {
            let trimmed = l.trim();
            if trimmed.is_empty() {
                return false;
            }
            let path_to_check = if let Some((_op, rest)) = trimmed.split_once(' ') {
                rest.trim()
            } else {
                trimmed
            };
            !project.is_ignored_or_cache_path(path_to_check)
        })
    }

    /// Ensures an isolated Jujutsu workspace exists for the specified agent.
    ///
    /// Creates `.tkg/workspaces/<agent>` if it does not already exist.
    /// If it already exists and is clean, automatically synchronizes it by branching
    /// a new working copy commit off the latest `default@` revision.
    /// Links `.tkg/workspaces/<agent>/target` to `<workspace_root>/target` to share build artifacts.
    /// Returns the canonical path to the isolated workspace.
    ///
    /// # Errors
    /// Returns `TkgError` if Jujutsu is not installed, not initialized, or workspace creation fails.
    #[allow(
        clippy::print_stderr,
        reason = "Diagnostic advisories when Jujutsu or Git prerequisites are missing"
    )]
    fn verify_jj_prerequisites(repo_root: &Path) -> Result<()> {
        if !is_jj_in_path() {
            eprintln!(
                "[tkg-common] Advisory: 'jj' (Jujutsu) CLI not found in PATH; cannot initialize isolated agent workspace."
            );
            return Err(TkgError::Process(
                "Jujutsu 'jj' CLI binary not found in PATH".to_owned(),
            ));
        }

        if !Self::is_jj_repo(repo_root) {
            if repo_root.join(".git").exists() {
                eprintln!(
                    "[tkg-common] Advisory: Target workspace '{}' is a Git repository without Jujutsu colocation.\n\
                     Run 'jj git init --colocate' in the repository root to enable isolated agent workspaces.",
                    repo_root.display()
                );
                return Err(TkgError::Workspace(format!(
                    "Target workspace '{}' is a Git repository without Jujutsu colocation. \
                     Run 'jj git init --colocate' in the repository root to enable isolated agent workspaces.",
                    repo_root.display()
                )));
            }
            eprintln!(
                "[tkg-common] Advisory: Target workspace '{}' is not a Jujutsu repository (.jj not found).\n\
                 Run 'jj git init --colocate' (for Git projects) or 'jj init' to enable isolated agent workspaces.",
                repo_root.display()
            );
            return Err(TkgError::Workspace(format!(
                "Target workspace '{}' is not a Jujutsu repository (.jj not found). \
                 Run 'jj git init --colocate' or 'jj init' to enable isolated agent workspaces.",
                repo_root.display()
            )));
        }

        Ok(())
    }

    #[allow(
        clippy::print_stderr,
        reason = "Diagnostic logging for subprocess rebase warnings"
    )]
    fn create_or_sync_workspace(repo_root: &Path, agent_ws_path: &Path, agent: &str) -> Result<()> {
        if !Self::workspace_exists(repo_root, agent) {
            let dest_str = agent_ws_path.to_str().unwrap_or(agent);
            let mut cmd = Command::new("jj");
            cmd.args([
                "--color=never",
                "workspace",
                "add",
                "--name",
                agent,
                dest_str,
            ]);
            cmd.current_dir(repo_root);
            let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
                .map_err(|e| {
                    TkgError::Process(format!("Failed to execute 'jj workspace add': {e}"))
                })?;

            if !output.status.success() {
                return Err(TkgError::Process(format!(
                    "Command 'jj workspace add --name {agent} {}' exited with non-zero status",
                    agent_ws_path.display()
                )));
            }
        } else if Self::is_workspace_clean(agent_ws_path) {
            // Rebase clean workspace onto latest default@ commit to eliminate split-brain drift
            let mut cmd = Command::new("jj");
            cmd.args(["--color=never", "new", "default@", "--no-edit"]);
            cmd.current_dir(agent_ws_path);
            match SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS) {
                Ok(out) if !out.status.success() => {
                    eprintln!(
                        "[tkg-common] warning: 'jj new default@' in {} exited with non-zero status: {}",
                        agent_ws_path.display(),
                        out.status
                    );
                },
                Err(e) => {
                    eprintln!(
                        "[tkg-common] warning: failed to execute 'jj new default@' in {}: {e}",
                        agent_ws_path.display()
                    );
                },
                _ => {},
            }
        }

        Ok(())
    }

    #[allow(clippy::print_stderr, reason = "Diagnostic logging for symlink errors")]
    fn link_shared_build_caches(repo_root: &Path, agent_ws_path: &Path) {
        let project = crate::project::ProjectConfig::detect_or_load(repo_root);
        for cache_name in project.all_ignored_or_cache_paths() {
            let root_cache = repo_root.join(&cache_name);
            let ws_cache = agent_ws_path.join(&cache_name);
            if !ws_cache.exists() && root_cache.is_dir() {
                #[cfg(unix)]
                {
                    if let Some(parent) = ws_cache.parent() {
                        drop(std::fs::create_dir_all(parent));
                    }
                    if let Err(e) = std::os::unix::fs::symlink(&root_cache, &ws_cache)
                        && e.kind() != std::io::ErrorKind::AlreadyExists
                    {
                        eprintln!(
                            "[tkg-common] warning: failed to symlink cache directory from {} to {}: {e}",
                            root_cache.display(),
                            ws_cache.display()
                        );
                    }
                }
            }
        }
    }

    pub fn ensure_agent_workspace(workspace_root: &Path, agent: &str) -> Result<PathBuf> {
        if agent.is_empty()
            || agent.starts_with('-')
            || !agent
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(TkgError::Workspace(format!(
                "Invalid agent workspace name '{agent}': must start with an alphanumeric or underscore character"
            )));
        }

        let repo_root = crate::resolver::WorkspaceResolver::find_root(Some(workspace_root))
            .unwrap_or_else(|_| workspace_root.to_path_buf());

        Self::verify_jj_prerequisites(&repo_root)?;

        let paths = crate::state::TkgStatePaths::new(&repo_root);
        let workspaces_dir = paths.workspaces_dir();
        std::fs::create_dir_all(&workspaces_dir)?;
        let agent_ws_path = paths.agent_workspace(agent);

        Self::create_or_sync_workspace(&repo_root, &agent_ws_path, agent)?;
        Self::link_shared_build_caches(&repo_root, &agent_ws_path);

        if agent_ws_path.is_dir() {
            agent_ws_path.canonicalize().map_err(TkgError::Io)
        } else {
            Err(TkgError::Workspace(format!(
                "Workspace directory '{}' was not found after creation",
                agent_ws_path.display()
            )))
        }
    }

    /// Forgets an agent's workspace and deletes its directory from disk.
    ///
    /// # Errors
    /// Returns `TkgError` if workspace forget fails.
    #[allow(
        clippy::print_stderr,
        reason = "Diagnostic logging for subprocess errors"
    )]
    pub fn forget_agent_workspace(workspace_root: &Path, agent: &str) -> Result<()> {
        let ws_path = crate::state::TkgStatePaths::new(workspace_root).agent_workspace(agent);
        if Self::workspace_exists(workspace_root, agent) {
            let status = Command::new("jj")
                .args(["--color=never", "workspace", "forget", agent])
                .current_dir(workspace_root)
                .status()
                .map_err(|e| {
                    TkgError::Process(format!(
                        "Failed to execute 'jj workspace forget {agent}': {e}"
                    ))
                })?;
            if !status.success() {
                eprintln!(
                    "[tkg-common] warning: 'jj workspace forget {agent}' exited with non-zero status: {status}"
                );
            }
        }
        if ws_path.exists() {
            std::fs::remove_dir_all(&ws_path).map_err(TkgError::Io)?;
        }
        Ok(())
    }

    /// Inspects the revisions and working-copy diff for an agent's isolated workspace.
    ///
    /// # Errors
    /// Returns `TkgError` if Jujutsu is not available or the workspace does not exist.
    pub fn inspect_agent_workspace(workspace_root: &Path, agent: &str) -> Result<String> {
        if agent.is_empty()
            || agent.starts_with('-')
            || !agent
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(TkgError::Workspace(format!(
                "Invalid agent workspace name '{agent}': must start with an alphanumeric or underscore character"
            )));
        }

        let repo_root = crate::resolver::WorkspaceResolver::find_root(Some(workspace_root))
            .unwrap_or_else(|_| workspace_root.to_path_buf());

        if !is_jj_in_path() {
            return Err(TkgError::Process(
                "Jujutsu 'jj' CLI binary not found in PATH".to_owned(),
            ));
        }

        if !Self::is_jj_repo(&repo_root) {
            return Err(TkgError::Workspace(format!(
                "Target workspace '{}' is not a Jujutsu repository (.jj not found).",
                repo_root.display()
            )));
        }

        let paths = crate::state::TkgStatePaths::new(&repo_root);
        let agent_ws_path = paths.agent_workspace(agent);

        if !agent_ws_path.is_dir() || !Self::workspace_exists(&repo_root, agent) {
            return Err(TkgError::Workspace(format!(
                "No isolated workspace found for agent '{agent}' at '{}'",
                agent_ws_path.display()
            )));
        }

        // Snapshot working copy in agent workspace
        snapshot_workspace(&agent_ws_path);

        // Retrieve diff
        let mut diff_cmd = Command::new("jj");
        diff_cmd.args(["--color=never", "diff"]);
        diff_cmd.current_dir(&agent_ws_path);
        let diff_out =
            SubprocessSupervisor::run_supervised(diff_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
                .map_err(|e| {
                    TkgError::Process(format!(
                        "Failed to execute 'jj diff' in {}: {e}",
                        agent_ws_path.display()
                    ))
                })?;
        let diff_str = String::from_utf8_lossy(&diff_out.stdout).trim().to_owned();

        // Retrieve revisions log
        let mut log_cmd = Command::new("jj");
        log_cmd.args([
            "--color=never",
            "log",
            "-r",
            "roots(default@..@)::@",
            "--no-graph",
        ]);
        log_cmd.current_dir(&agent_ws_path);
        let log_out = SubprocessSupervisor::run_supervised(log_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if diff_str.is_empty() && (log_out.is_empty() || log_out.contains("(empty)")) {
            return Ok(format!(
                "Agent workspace '{agent}' is clean (0 uncommitted or branched changes)."
            ));
        }

        let mut output = format!(
            "=== Isolated Workspace: '{agent}' ({}) ===\n",
            agent_ws_path.display()
        );
        if !log_out.is_empty() {
            output.push_str("--- Revisions ---\n");
            output.push_str(&log_out);
            output.push('\n');
        }
        if !diff_str.is_empty() {
            output.push_str("--- Working Copy Diff ---\n");
            output.push_str(&diff_str);
            output.push('\n');
        }
        Ok(output)
    }

    /// Merges an agent's isolated workspace back into the main working tree `@`.
    ///
    /// 1. Snapshots starting VCS operation for rollback.
    /// 2. Snapshots subagent workspace working copy.
    /// 3. Rebases subagent revision(s) onto `@` (or squashes into `@` if strategy is 'squash').
    /// 4. Advances main working copy on top of rebased revision via `jj new <agent>@`.
    /// 5. Verifies zero merge conflicts (`conflicts() & @::`).
    /// 6. Evaluates declarative project quality gates (`project.toml`) on the merged tree.
    /// 7. On gate failure, restores starting VCS operation via `jj op restore`.
    /// 8. On gate pass, cleans up the isolated workspace via `forget_agent_workspace`.
    ///
    /// # Errors
    /// Returns `TkgError` if rebase, conflicts, or quality gates fail.
    #[allow(
        clippy::too_many_lines,
        reason = "Comprehensive VCS merge and gate workflow"
    )]
    pub fn merge_agent_workspace(
        workspace_root: &Path,
        agent: &str,
        strategy: Option<&str>,
    ) -> Result<String> {
        if agent.is_empty()
            || agent.starts_with('-')
            || !agent
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(TkgError::Workspace(format!(
                "Invalid agent workspace name '{agent}': must start with an alphanumeric or underscore character"
            )));
        }

        let strat = strategy.unwrap_or("rebase");
        if strat != "rebase" && strat != "squash" {
            return Err(TkgError::Workspace(format!(
                "Invalid merge strategy '{strat}': expected 'rebase' or 'squash'"
            )));
        }

        let repo_root = crate::resolver::WorkspaceResolver::find_root(Some(workspace_root))
            .unwrap_or_else(|_| workspace_root.to_path_buf());

        if !is_jj_in_path() {
            return Err(TkgError::Process(
                "Jujutsu 'jj' CLI binary not found in PATH".to_owned(),
            ));
        }

        if !Self::is_jj_repo(&repo_root) {
            return Err(TkgError::Workspace(format!(
                "Target workspace '{}' is not a Jujutsu repository (.jj not found).",
                repo_root.display()
            )));
        }

        let paths = crate::state::TkgStatePaths::new(&repo_root);
        let agent_ws_path = paths.agent_workspace(agent);

        if !agent_ws_path.is_dir() || !Self::workspace_exists(&repo_root, agent) {
            return Err(TkgError::Workspace(format!(
                "No isolated workspace found for agent '{agent}' at '{}'",
                agent_ws_path.display()
            )));
        }

        // Snapshot agent workspace
        snapshot_workspace(&agent_ws_path);

        // Check if there are any changes to merge
        let is_clean = Self::is_workspace_clean(&agent_ws_path);
        let mut log_revs_cmd = Command::new("jj");
        log_revs_cmd.args([
            "--color=never",
            "log",
            "-r",
            &format!("default@..{agent}@"),
            "--no-graph",
            "-T",
            "id\n",
        ]);
        log_revs_cmd.current_dir(&repo_root);
        let revs_out =
            SubprocessSupervisor::run_supervised(log_revs_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
                .map_or_default(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned());

        if is_clean && revs_out.is_empty() {
            return Ok(format!(
                "Isolated workspace '{agent}' has no changes to merge into '@'."
            ));
        }

        let start_op_id = query_current_op_id(&repo_root);

        if let Err(e) = execute_merge_vcs_steps(&repo_root, agent, strat) {
            restore_op(&repo_root, start_op_id.as_deref());
            return Err(e);
        }

        // Check for conflicts
        if let Err(conf_err) = check_conflicts(&repo_root, agent, &agent_ws_path) {
            restore_op(&repo_root, start_op_id.as_deref());
            return Err(conf_err);
        }

        // Evaluate declarative project quality gates on merged tree
        let project = crate::project::ProjectConfig::detect_or_load(&repo_root);
        let mut gate_names: Vec<&str> = project.gates.keys().map(String::as_str).collect();
        gate_names.push("vcs_conflicts");

        match crate::gatekeeper::evaluate_pre_flight_checks(&repo_root, &gate_names) {
            Ok(warnings) => {
                Self::forget_agent_workspace(&repo_root, agent)?;

                let mut report = format!(
                    "✅ Successfully merged isolated workspace for '{agent}' onto '@' (strategy: {strat}):\n\
                     - Quality gates passed: {}\n\
                     - Workspace '{}' forgotten and cleaned up.",
                    if gate_names.is_empty() {
                        "none configured".to_owned()
                    } else {
                        gate_names.join(", ")
                    },
                    agent_ws_path.display()
                );
                if !warnings.is_empty() {
                    report.push_str("\n\nAdvisories:\n");
                    for w in warnings {
                        report.push_str("- ");
                        report.push_str(&w);
                        report.push('\n');
                    }
                }
                Ok(report)
            },
            Err(errors) => {
                restore_op(&repo_root, start_op_id.as_deref());
                Err(TkgError::Process(format!(
                    "Merge aborted: Quality gate evaluation failed on merged tree:\n{}\n\
                     Merge rolled back via 'jj op restore'. Workspace preserved at '{}'.",
                    errors.join("\n"),
                    agent_ws_path.display()
                )))
            },
        }
    }

    /// Collects working copy diff summary lines (e.g. `M src/main.rs`),
    /// filtered by `project.is_ignored_or_cache_path()`.
    /// Executes under `SubprocessSupervisor` timeout to prevent hanging on locks.
    #[must_use]
    pub fn get_working_copy_diff_summary(
        root: &Path,
        project: &crate::project::ProjectConfig,
    ) -> String {
        let lines = Self::get_working_copy_status_lines(root, project);
        lines.join("\n")
    }

    /// Collects modified file paths in the working copy (via `jj diff --summary` or `git status -s`),
    /// filtered by `project.is_ignored_or_cache_path()`.
    /// Executes under `SubprocessSupervisor` timeout to prevent hanging on locks.
    #[must_use]
    pub fn get_modified_files(
        root: &Path,
        project: &crate::project::ProjectConfig,
        max_files: usize,
    ) -> Vec<String> {
        let lines = Self::get_working_copy_status_lines(root, project);
        let mut files = Vec::new();
        for line in lines {
            let path_to_check = if let Some((_, p)) = line.split_once(' ') {
                p.trim()
            } else {
                line.trim()
            };
            if !path_to_check.is_empty() {
                files.push(path_to_check.to_owned());
                if files.len() >= max_files {
                    break;
                }
            }
        }
        files
    }

    /// Helper retrieving filtered raw status lines from Jujutsu or Git under supervision.
    fn get_working_copy_status_lines(
        root: &Path,
        project: &crate::project::ProjectConfig,
    ) -> Vec<String> {
        let raw_output = if Self::is_jj_repo(root) {
            let mut cmd = Command::new("jj");
            cmd.args(["--color=never", "diff", "--summary"]);
            cmd.current_dir(root);
            SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
        } else {
            let mut cmd = Command::new("git");
            cmd.args(["status", "-s"]);
            cmd.current_dir(root);
            SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
        };

        let mut filtered = Vec::new();
        if let Ok(output) = raw_output
            && output.status.success()
        {
            let stdout_lossy = String::from_utf8_lossy(&output.stdout);
            for line in stdout_lossy.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let path_to_check = if let Some((_, p)) = trimmed.split_once(' ') {
                    p.trim()
                } else {
                    trimmed
                };
                if !project.is_ignored_or_cache_path(path_to_check) {
                    filtered.push(trimmed.to_owned());
                }
            }
        }
        filtered
    }
}

fn snapshot_workspace(ws_path: &Path) {
    let mut status_cmd = Command::new("jj");
    status_cmd.args(["--color=never", "status"]);
    status_cmd.current_dir(ws_path);
    drop(SubprocessSupervisor::run_supervised(
        status_cmd,
        DEFAULT_VCS_QUERY_TIMEOUT_SECS,
    ));
}

fn query_current_op_id(repo_root: &Path) -> Option<String> {
    let mut op_cmd = Command::new("jj");
    op_cmd.args([
        "--color=never",
        "op",
        "log",
        "-n",
        "1",
        "--no-graph",
        "-T",
        "id",
    ]);
    op_cmd.current_dir(repo_root);
    SubprocessSupervisor::run_supervised(op_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
            } else {
                None
            }
        })
        .filter(|id| !id.is_empty())
}

fn restore_op(repo_root: &Path, op_id: Option<&str>) {
    if let Some(id) = op_id {
        let mut restore_cmd = Command::new("jj");
        restore_cmd.args(["--color=never", "op", "restore", id]);
        restore_cmd.current_dir(repo_root);
        drop(SubprocessSupervisor::run_supervised(
            restore_cmd,
            DEFAULT_VCS_QUERY_TIMEOUT_SECS,
        ));
    }
}

fn execute_merge_vcs_steps(repo_root: &Path, agent: &str, strategy: &str) -> Result<()> {
    if strategy == "squash" {
        let mut squash_cmd = Command::new("jj");
        squash_cmd.args([
            "--color=never",
            "squash",
            "--from",
            &format!("{agent}@"),
            "--into",
            "@",
        ]);
        squash_cmd.current_dir(repo_root);
        let squash_res =
            SubprocessSupervisor::run_supervised(squash_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
                .map_err(|e| TkgError::Process(format!("Failed to execute 'jj squash': {e}")))?;
        if !squash_res.status.success() {
            let stderr = String::from_utf8_lossy(&squash_res.stderr);
            return Err(TkgError::Process(format!(
                "'jj squash --from {agent}@ --into @' failed: {stderr}"
            )));
        }
    } else {
        // Rebase revision(s) onto @
        let roots_expr = format!("roots(default@..{agent}@)");
        let mut rebase_cmd = Command::new("jj");
        rebase_cmd.args(["--color=never", "rebase", "-s", &roots_expr, "-d", "@"]);
        rebase_cmd.current_dir(repo_root);
        let rebase_res =
            SubprocessSupervisor::run_supervised(rebase_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS);
        let rebase_success = match rebase_res {
            Ok(ref out) if out.status.success() => true,
            _ => {
                let mut fallback_cmd = Command::new("jj");
                fallback_cmd.args([
                    "--color=never",
                    "rebase",
                    "-s",
                    &format!("{agent}@"),
                    "-d",
                    "@",
                ]);
                fallback_cmd.current_dir(repo_root);
                SubprocessSupervisor::run_supervised(fallback_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
                    .is_ok_and(|out| out.status.success())
            },
        };
        if !rebase_success {
            return Err(TkgError::Process(format!(
                "Failed to rebase revisions from agent '{agent}' onto '@'"
            )));
        }

        // Create new working copy commit on top of rebased changes
        let mut new_cmd = Command::new("jj");
        new_cmd.args(["--color=never", "new", &format!("{agent}@")]);
        new_cmd.current_dir(repo_root);
        let new_res = SubprocessSupervisor::run_supervised(new_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
            .map_err(|e| {
                TkgError::Process(format!("Failed to advance working copy via 'jj new': {e}"))
            })?;
        if !new_res.status.success() {
            let stderr = String::from_utf8_lossy(&new_res.stderr);
            return Err(TkgError::Process(format!(
                "'jj new {agent}@' failed: {stderr}"
            )));
        }
    }
    Ok(())
}

fn check_conflicts(repo_root: &Path, _agent: &str, agent_ws_path: &Path) -> Result<()> {
    let mut conf_cmd = Command::new("jj");
    conf_cmd.args([
        "--color=never",
        "log",
        "-r",
        "conflicts() & @::",
        "--no-graph",
    ]);
    conf_cmd.current_dir(repo_root);
    if let Ok(conf_out) =
        SubprocessSupervisor::run_supervised(conf_cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
    {
        let conf_str = String::from_utf8_lossy(&conf_out.stdout).trim().to_owned();
        if !conf_str.is_empty() {
            return Err(TkgError::Process(format!(
                "Merge aborted: Unresolved conflicts detected in rebased tree:\n{conf_str}\n\
                 Merge rolled back via 'jj op restore'. Workspace preserved at '{}'.",
                agent_ws_path.display()
            )));
        }
    }
    Ok(())
}

fn is_jj_in_path() -> bool {
    if let Some(paths) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&paths) {
            if p.join("jj").is_file() {
                return true;
            }
        }
    }
    false
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
    fn test_is_jj_repo_on_current_workspace() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        if JujutsuWorkspace::is_jj_repo(&root) {
            let _clean = JujutsuWorkspace::is_workspace_clean(&root);
        }

        let temp_dir = std::env::temp_dir().join(format!("tkg_jj_test_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp_dir));
        std::fs::create_dir_all(temp_dir.join(".jj")).expect("create temp .jj dir");
        assert!(JujutsuWorkspace::is_jj_repo(&temp_dir));
        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_workspace_exists_default() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        if JujutsuWorkspace::is_jj_repo(&root) {
            assert!(JujutsuWorkspace::workspace_exists(&root, "default"));
        }
        assert!(!JujutsuWorkspace::workspace_exists(
            &root,
            "non_existent_workspace_12345"
        ));
    }

    #[test]
    #[allow(clippy::assertions_on_result_states, reason = "Test error state")]
    fn test_inspect_agent_workspace_non_existent() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let res = JujutsuWorkspace::inspect_agent_workspace(&root, "non_existent_agent_999");
        assert!(res.is_err());
    }

    #[test]
    #[allow(clippy::assertions_on_result_states, reason = "Test error state")]
    fn test_merge_agent_workspace_non_existent() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let res = JujutsuWorkspace::merge_agent_workspace(&root, "non_existent_agent_999", None);
        assert!(res.is_err());
    }

    #[test]
    fn test_merge_agent_workspace_invalid_strategy() {
        let root = Path::new(".");
        let res =
            JujutsuWorkspace::merge_agent_workspace(root, "test_agent", Some("invalid_strat"));
        assert!(res.is_err_and(|e| {
            e.to_string()
                .contains("Invalid merge strategy 'invalid_strat'")
        }));
    }
}
