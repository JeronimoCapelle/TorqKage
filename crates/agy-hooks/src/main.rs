//! # agy-hooks
//!
//! Dedicated featherweight CLI binary (<500KB, <1ms startup) handling Antigravity lifecycle events.

#![allow(clippy::print_stdout, reason = "CLI hook tool emits ProtoJSON directly to stdout")]
#![allow(clippy::print_stderr, reason = "CLI diagnostics are emitted to stderr")]

mod conflicts;
mod finisher_gate;
mod handoff;
mod policy_gate;
mod state;
mod stdin;
mod wip;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map_or("help", String::as_str);

    match command {
        "detect-wip" => {
            wip::execute();
            ExitCode::SUCCESS
        }
        "check-conflicts" => {
            conflicts::execute();
            ExitCode::SUCCESS
        }
        "check-finisher" => {
            finisher_gate::execute();
            ExitCode::SUCCESS
        }
        "enforce-tool-policy" => {
            policy_gate::execute();
            ExitCode::SUCCESS
        }
        "record-handoff" => {
            handoff::execute_record_handoff();
            ExitCode::SUCCESS
        }
        "inject-wake" => {
            handoff::execute_inject_wake();
            ExitCode::SUCCESS
        }
        "inspect-state" => {
            state::execute();
            ExitCode::SUCCESS
        }
        "reset-circuit-breaker" => {
            let root = agy_common::WorkspaceResolver::find_root(None).unwrap_or_else(|_| std::path::PathBuf::from("."));
            let mut ledger = agy_common::CircuitBreakerLedger::load(&root);
            if ledger.reset(&root).is_ok() {
                println!("Circuit breaker reset successfully.");
                ExitCode::SUCCESS
            } else {
                eprintln!("Failed to reset circuit breaker.");
                ExitCode::FAILURE
            }
        }
        "ensure-workspace" => {
            let agent = match args.get(2) {
                Some(a) if !a.trim().is_empty() => a.trim(),
                _ => {
                    eprintln!("Usage: agy-hooks ensure-workspace <agent-name>");
                    return ExitCode::FAILURE;
                }
            };
            let root = agy_common::WorkspaceResolver::find_root(None).unwrap_or_else(|_| std::path::PathBuf::from("."));
            match agy_common::JujutsuWorkspace::ensure_agent_workspace(&root, agent) {
                Ok(path) => {
                    println!("✔ Isolated workspace for agent '{agent}' ready at: {}", path.display());
                    println!("  Target cache symlinked to shared root target; clean revisions rebased onto default@.");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("Failed to ensure agent workspace: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "check-clean" => {
            let target_path = args.get(2).map_or_else(
                || agy_common::WorkspaceResolver::find_root(None).unwrap_or_else(|_| std::path::PathBuf::from(".")),
                std::path::PathBuf::from,
            );
            if agy_common::JujutsuWorkspace::is_workspace_clean(&target_path) {
                println!("✔ Workspace at '{}' is clean (0 uncommitted modifications).", target_path.display());
                ExitCode::SUCCESS
            } else {
                println!("⚠ Workspace at '{}' has uncommitted changes:", target_path.display());
                drop(std::process::Command::new("jj")
                    .args(["--color=never", "diff", "--summary"])
                    .current_dir(&target_path)
                    .status());
                ExitCode::FAILURE
            }
        }
        "help" | "--help" | "-h" => {
            print_usage();
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("agy-hooks: unknown command '{command}'");
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!("agy-hooks: Antigravity native lifecycle hook runner");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  agy-hooks <COMMAND>");
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!("  detect-wip            PreInvocation ephemeral notification on working tree modification");
    eprintln!("  check-conflicts       PreToolUse hook blocking builds/tests when unresolved conflicts exist");
    eprintln!("  check-finisher        PreToolUse hook blocking finisher dispatch when compilation or conflicts fail");
    eprintln!("  enforce-tool-policy   PreToolUse hook enforcing granular ASK vs ALLOW decision per tool");
    eprintln!("  record-handoff        Stop hook recording session metadata to .pipeline/last_handoff.md");
    eprintln!("  inject-wake           SessionStart hook injecting last handoff state into conversation");
    eprintln!("  inspect-state         Concise Markdown repository state summary for tools.toml");
    eprintln!("  reset-circuit-breaker Reset the 2-strike compiler failure circuit breaker");
    eprintln!("  ensure-workspace      Ensure isolated Jujutsu agent workspace exists with shared target");
    eprintln!("  check-clean           Verify workspace has zero uncommitted changes");
}
