//! In-process project state inspection for the MCP daemon.

use std::path::Path;
use std::process::Command;
use agy_common::AgyStatePaths;
use agy_common::JujutsuWorkspace;

pub(crate) fn execute(workspace_root: &Path) -> (String, bool) {
    let branch = query_current_branch(workspace_root);
    let git_status = query_vcs_status(workspace_root);
    let handoff = read_last_handoff(workspace_root);

    let mut out = format!("# Project State: Ready\n- Branch: {branch}\n- Working Copy: {git_status}\n");
    if let Some(h) = handoff {
        out.push_str("- Last Handoff: ");
        out.push_str(&h);
        out.push('\n');
    }
    (out, false)
}

fn query_current_branch(root: &Path) -> String {
    if JujutsuWorkspace::is_jj_repo(root) {
        if let Ok(out) = Command::new("jj")
            .args(["--color=never", "log", "-r", "@", "--no-graph", "-T", "bookmarks"])
            .current_dir(root)
            .output()
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

    if let Ok(out) = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
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
        if let Ok(out) = Command::new("jj")
            .args(["--color=never", "diff", "--summary"])
            .current_dir(root)
            .output()
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

    if let Ok(out) = Command::new("git")
        .args(["status", "-s"])
        .current_dir(root)
        .output()
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
}
