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
    eprintln!(
        "agy-hooks: Antigravity native lifecycle hook runner\n\
         \n\
         USAGE:\n\
           agy-hooks <COMMAND>\n\
         \n\
         COMMANDS:\n\
           detect-wip      PreInvocation ephemeral notification on working tree modification\n\
           check-conflicts PreToolUse hook blocking builds/tests when unresolved conflicts exist\n\
           check-finisher  PreToolUse hook blocking finisher dispatch when compilation or conflicts fail\n\
           enforce-tool-policy PreToolUse hook enforcing granular ASK vs ALLOW decision per tool\n\
           record-handoff  Stop hook recording session metadata to .pipeline/last_handoff.md\n\
           inject-wake     SessionStart hook injecting last handoff state into conversation\n\
           inspect-state   Concise Markdown repository state summary for tools.toml\n\
           reset-circuit-breaker Reset the 2-strike compiler failure circuit breaker\n"
    );
}
