//! `PreToolUse` compiler cleanliness verification hook (`cargo clippy`).

use std::path::{Path, PathBuf};
use std::process::Command;
use agy_common::{PreToolResponse, PreToolUseInput, WorkspaceResolver};
use crate::stdin::read_stdin_payload;

/// Executes the `check-clippy` hook.
///
/// Runs `cargo clippy --all-targets -- -D warnings --quiet` against the workspace root.
/// Emits `allow` if clean, or `deny` with actionable diagnostics if warnings or errors are found.
pub(crate) fn execute() {
    let input: PreToolUseInput = read_stdin_payload();

    let root = resolve_workspace_root(&input);

    let resp = run_clippy_check(&root);

    if let Ok(json) = serde_json::to_string(&resp) {
        println!("{json}");
    }
}

fn resolve_workspace_root(input: &PreToolUseInput) -> PathBuf {
    if let Some(first_path) = input.common.workspace_paths.first()
        && first_path.exists()
    {
        return first_path.clone();
    }

    WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."))
}

fn run_clippy_check(root: &Path) -> PreToolResponse {
    let cargo_manifest = root.join("Cargo.toml");
    if !cargo_manifest.exists() {
        return PreToolResponse::allow();
    }

    let output_result = Command::new("cargo")
        .args(["clippy", "--all-targets", "--", "-D", "warnings", "--quiet"])
        .current_dir(root)
        .output();

    match output_result {
        Ok(output) => {
            if output.status.success() {
                PreToolResponse::allow()
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let diagnostics = if stderr.trim().is_empty() {
                    stdout.trim()
                } else {
                    stderr.trim()
                };

                let sample_lines: Vec<&str> = diagnostics.lines().take(5).collect();
                let snippet = sample_lines.join("\n");

                let reason = format!(
                    "GATE BLOCKED: cargo clippy failed with 1+ warnings. Run 'cargo clippy --fix' first.\n{snippet}"
                );
                PreToolResponse::deny(reason)
            }
        }
        Err(err) => {
            PreToolResponse::deny(format!("GATE ERROR: unable to execute cargo clippy: {err}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_cargo_root_allows() {
        let temp_dir = std::env::temp_dir();
        let resp = run_clippy_check(&temp_dir);
        assert_eq!(resp.decision, agy_common::HookDecision::Allow);
    }
}
