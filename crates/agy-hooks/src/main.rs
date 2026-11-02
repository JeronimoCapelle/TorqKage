//! # agy-hooks
//!
//! Dedicated featherweight CLI binary (<500KB, <1ms startup) handling Antigravity lifecycle events.
//!
//! ## Overview
//! `agy-hooks` is invoked ephemerally by the Antigravity agent runtime during lifecycle events
//! (`PreInvocation`, `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`). It exchanges `ProtoJSON`
//! payloads over `stdin`/`stdout` without background daemon overhead.
//!
//! ## Subcommands
//! - `detect-wip`: Emits ephemeral notification on working tree modification; burns zero tokens if clean.
//! - `check-conflicts`: Blocks builds and tests if unresolved Jujutsu or Git conflict markers exist.
//! - `check-subagent`: Intercepts `invoke_subagent` and evaluates declared `pre_launch_checks` (alias: `check-finisher`).
//! - `enforce-tool-policy`: Enforces operator confirmation (`ask`) for mutating tools or tripped circuit breaker.
//! - `record-handoff`: Captures session state and modified files to `.pipeline/` upon session termination.
//! - `inject-wake`: Injects prior handoff state into Turn 1 of new conversation sessions.
//! - `reset-circuit-breaker`: Resets the 2-strike compiler failure ledger.
//! - `ensure-workspace`: Configures an isolated Jujutsu workspace with shared target build cache.
//! - `check-clean`: Verifies that a workspace has zero uncommitted modifications.

#![allow(clippy::print_stdout, reason = "CLI hook tool emits ProtoJSON directly to stdout")]
#![allow(clippy::print_stderr, reason = "CLI diagnostics are emitted to stderr")]

mod config;
mod conflicts;
mod finisher_gate;
mod handoff;
mod policy_gate;
mod pre_tool_use;
mod state;
mod stdin;
mod wip;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map_or("help", String::as_str);

    match command {
        "pre-tool-use" | "tool-use" => {
            pre_tool_use::execute();
            ExitCode::SUCCESS
        }
        "detect-wip" => {
            wip::execute();
            ExitCode::SUCCESS
        }
        "check-conflicts" => {
            conflicts::execute();
            ExitCode::SUCCESS
        }
        "check-subagent" | "check-finisher" => {
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
            eprintln!("[agy-hooks] Advisory: 'inspect-state' in agy-hooks is deprecated; use agy-mcp tool 'inspect_project_state' instead.");
            state::execute();
            ExitCode::SUCCESS
        }
        "reset-circuit-breaker" => {
            eprintln!("[agy-hooks] Advisory: 'reset-circuit-breaker' in agy-hooks is deprecated; use 'agy-ctrl reset-breaker' instead.");
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
            eprintln!("[agy-hooks] Advisory: 'ensure-workspace' in agy-hooks is deprecated; use agy-mcp tool 'ensure_agent_workspace' or 'agy-ctrl workspace ensure' instead.");
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
            eprintln!("[agy-hooks] Advisory: 'check-clean' in agy-hooks is deprecated; use 'jj diff --summary' instead.");
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
    eprintln!("  pre-tool-use          Unified hybrid PreToolUse hook evaluating hooks.toml pipeline (alias: tool-use)");
    eprintln!("  detect-wip            PreInvocation ephemeral notification on working tree modification");
    eprintln!("  check-conflicts       PreToolUse hook blocking builds/tests when unresolved conflicts exist");
    eprintln!("  check-subagent        PreToolUse hook blocking subagent dispatch when pre-launch quality gates fail (alias: check-finisher)");
    eprintln!("  enforce-tool-policy   PreToolUse hook enforcing granular ASK vs ALLOW decision per tool");
    eprintln!("  record-handoff        Stop hook recording session metadata to .pipeline/last_handoff.md");
    eprintln!("  inject-wake           SessionStart hook injecting last handoff state into conversation");
    eprintln!("  inspect-state         Concise Markdown repository state summary for tools.toml");
    eprintln!("  reset-circuit-breaker Reset the 2-strike compiler failure circuit breaker");
    eprintln!("  ensure-workspace      Ensure isolated Jujutsu agent workspace exists with shared target");
    eprintln!("  check-clean           Verify workspace has zero uncommitted changes");
}
