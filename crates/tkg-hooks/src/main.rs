//! # tkg-hooks
//!
//! Dedicated featherweight CLI binary (<500KB, <1ms startup) handling Antigravity lifecycle events.
//!
//! ## Overview
//! `tkg-hooks` is invoked ephemerally by the Antigravity agent runtime during lifecycle events
//! (`PreInvocation`, `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`). It exchanges `ProtoJSON`
//! payloads over `stdin`/`stdout` without background daemon overhead.
//!
//! ## Subcommands
//! - `pre-tool-use`: Unified hybrid `PreToolUse` hook evaluating `hooks.toml` pipeline (alias: `tool-use`).
//! - `detect-wip`: Emits ephemeral notification on working tree modification; burns zero tokens if clean.
//! - `check-conflicts`: Blocks builds and tests if unresolved Jujutsu or Git conflict markers exist.
//! - `check-subagent`: Intercepts `invoke_subagent` and evaluates declared `pre_launch_checks`.
//! - `enforce-tool-policy`: Enforces operator confirmation (`ask`) for mutating tools or tripped circuit breaker.
//! - `record-handoff`: Captures session state and modified files to `.tkg/state/` upon session termination.
//! - `inject-wake`: Injects prior handoff state into Turn 1 of new conversation sessions.

#![forbid(unsafe_code)]
#![allow(
    clippy::print_stdout,
    reason = "CLI hook tool emits ProtoJSON directly to stdout"
)]
#![allow(clippy::print_stderr, reason = "CLI diagnostics are emitted to stderr")]

mod config;
mod conflicts;
mod handoff;
mod manifest_guard;
mod policy_gate;
mod pre_tool_use;
mod stdin;
mod subagent_gate;
mod wip;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map_or("help", String::as_str);

    match command {
        "pre-tool-use" | "tool-use" => {
            pre_tool_use::execute();
            ExitCode::SUCCESS
        },
        "detect-wip" => {
            wip::execute();
            ExitCode::SUCCESS
        },
        "check-conflicts" => {
            conflicts::execute();
            ExitCode::SUCCESS
        },
        "check-subagent" => {
            subagent_gate::execute();
            ExitCode::SUCCESS
        },
        "enforce-tool-policy" => {
            policy_gate::execute();
            ExitCode::SUCCESS
        },
        "record-handoff" => {
            handoff::execute_record_handoff();
            ExitCode::SUCCESS
        },
        "inject-wake" => {
            handoff::execute_inject_wake();
            ExitCode::SUCCESS
        },
        "help" | "--help" | "-h" => {
            print_usage();
            ExitCode::SUCCESS
        },
        _ => {
            eprintln!("tkg-hooks: unknown command '{command}'");
            print_usage();
            ExitCode::FAILURE
        },
    }
}

fn print_usage() {
    eprintln!("tkg-hooks: Antigravity native lifecycle hook runner");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("  tkg-hooks <COMMAND>");
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!(
        "  pre-tool-use          Unified hybrid PreToolUse hook evaluating hooks.toml pipeline (alias: tool-use)"
    );
    eprintln!(
        "  detect-wip            PreInvocation ephemeral notification on working tree modification"
    );
    eprintln!(
        "  check-conflicts       PreToolUse hook blocking builds/tests when unresolved conflicts exist"
    );
    eprintln!(
        "  check-subagent        PreToolUse hook blocking subagent dispatch when pre-launch quality gates fail"
    );
    eprintln!(
        "  enforce-tool-policy   PreToolUse hook enforcing granular ASK vs ALLOW decision per tool"
    );
    eprintln!(
        "  record-handoff        Stop hook recording session metadata to .tkg/state/last_handoff.md"
    );
    eprintln!(
        "  inject-wake           SessionStart hook injecting last handoff state into conversation"
    );
}
