//! Repository state inspection helper for agent consumption (`inspect_state`).

use std::path::{Path, PathBuf};
use std::process::Command;
use agy_common::WorkspaceResolver;

/// Executes the `inspect-state` command.
///
/// Gathers active branch, working tree cleanliness, and last handoff record,
/// printing a concise Markdown summary (<20 lines) to `stdout`.
pub(crate) fn execute() {
    let root = WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."));

    let branch = query_current_branch(&root);
    let git_status = query_git_status(&root);
    let handoff_summary = read_last_handoff(&root);

    println!("# Project State: Ready");
    println!("- Branch: {branch}");
    println!("- Working Copy: {git_status}");
    if let Some(handoff) = handoff_summary {
        println!("- Last Handoff: {handoff}");
    }
}

fn query_current_branch(root: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let name = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if name.is_empty() {
                "detached".to_owned()
            } else {
                name
            }
        }
        _ => "unknown".to_owned(),
    }
}

fn query_git_status(root: &Path) -> String {
    let output = Command::new("git")
        .args(["status", "-s"])
        .current_dir(root)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let status = String::from_utf8_lossy(&out.stdout);
            let count = status.lines().count();
            if count == 0 {
                "Clean".to_owned()
            } else {
                format!("{count} modified file(s)")
            }
        }
        _ => "Unavailable".to_owned(),
    }
}

fn read_last_handoff(root: &Path) -> Option<String> {
    let handoff_path = root.join(".pipeline").join("last_handoff.md");
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
