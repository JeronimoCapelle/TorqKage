//! In-process project state inspection for the MCP daemon.

use std::{path::Path, process::Command};

use agy_common::{
    AgyStatePaths, DEFAULT_VCS_QUERY_TIMEOUT_SECS, JujutsuWorkspace, SubprocessSupervisor,
};

pub(crate) fn execute(workspace_root: &Path) -> (String, bool) {
    let branch = query_current_branch(workspace_root);
    let git_status = query_vcs_status(workspace_root);
    let handoff = read_last_handoff(workspace_root);
    let project = agy_common::ProjectConfig::detect_or_load(workspace_root);

    let mut out =
        format!("# Project State: Ready\n- Branch: {branch}\n- Working Copy: {git_status}\n");
    if !project.docs.paths.is_empty() {
        out.push_str("- Documentation & Specs: ");
        out.push_str(&project.docs.paths.join(", "));
        out.push_str(" (grep via 'query_project_spec')\n");
    }
    if let Some(h) = handoff {
        out.push_str("- Last Handoff: ");
        out.push_str(&h);
        out.push('\n');
    }
    (out, false)
}

fn query_current_branch(root: &Path) -> String {
    if JujutsuWorkspace::is_jj_repo(root) {
        let mut cmd = Command::new("jj");
        cmd.args([
            "--color=never",
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            "bookmarks",
        ]);
        cmd.current_dir(root);
        if let Ok(out) = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
            && out.status.success()
        {
            let bm = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if !bm.is_empty() {
                return bm;
            }
        }
        if let Some(ws_name) = JujutsuWorkspace::current_workspace_name(root) {
            return format!("jj workspace: {ws_name}");
        }
        return "@ (working copy)".to_owned();
    }

    let mut cmd = Command::new("git");
    cmd.args(["rev-parse", "--abbrev-ref", "HEAD"]);
    cmd.current_dir(root);
    if let Ok(out) = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
        && out.status.success()
    {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if !name.is_empty() {
            return name;
        }
    }
    "unknown".to_owned()
}

fn query_vcs_status(root: &Path) -> String {
    if JujutsuWorkspace::is_jj_repo(root) {
        if JujutsuWorkspace::is_workspace_clean(root) {
            return "Clean".to_owned();
        }
        let mut cmd = Command::new("jj");
        cmd.args(["--color=never", "diff", "--summary"]);
        cmd.current_dir(root);
        if let Ok(out) = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
            && out.status.success()
        {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let count = stdout
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.is_empty() && t != "A target" && t != "M target"
                })
                .count();
            return format!("{count} modified file(s)");
        }
        return "Unavailable".to_owned();
    }

    let mut cmd = Command::new("git");
    cmd.args(["status", "-s"]);
    cmd.current_dir(root);
    if let Ok(out) = SubprocessSupervisor::run_supervised(cmd, DEFAULT_VCS_QUERY_TIMEOUT_SECS)
        && out.status.success()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let count = stdout.lines().count();
        if count == 0 {
            "Clean".to_owned()
        } else {
            format!("{count} modified file(s)")
        }
    } else {
        "Unavailable".to_owned()
    }
}

fn read_last_handoff(root: &Path) -> Option<String> {
    let paths = AgyStatePaths::new(root);
    let handoff_path = paths.handoff_md_file();
    if handoff_path.exists()
        && let Ok(content) = std::fs::read_to_string(&handoff_path)
    {
        let first_line = content.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() {
            return Some(first_line.to_owned());
        }
    }
    None
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_project_state_basic_output() {
        let root = Path::new(".");
        let (output, is_error) = execute(root);
        assert!(!is_error);
        assert!(output.contains("# Project State: Ready"));
        assert!(output.contains("- Branch:"));
        assert!(output.contains("- Working Copy:"));
    }

    #[test]
    fn test_project_state_includes_docs_when_configured() {
        let temp = std::env::temp_dir().join(format!("agy_state_docs_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp));
        std::fs::create_dir_all(&temp).expect("create temp dir");
        std::fs::write(
            temp.join("project.toml"),
            "[project]\nname = \"test\"\n\n[docs]\npaths = [\"docs\", \"specs/rv32.md\"]\n",
        )
        .expect("write project.toml");

        let (output, is_error) = execute(&temp);
        assert!(!is_error);
        assert!(output.contains(
            "- Documentation & Specs: docs, specs/rv32.md (grep via 'query_project_spec')"
        ));

        drop(std::fs::remove_dir_all(&temp));
    }
}
