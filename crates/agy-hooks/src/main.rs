//! # agy-hooks
//!
//! Dedicated featherweight CLI binary (<500KB, <1ms startup) handling Antigravity lifecycle events.

#![allow(clippy::print_stdout, reason = "CLI hook tool emits ProtoJSON directly to stdout")]
#![allow(clippy::print_stderr, reason = "CLI diagnostics are emitted to stderr")]

mod handoff;
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
           record-handoff  Stop hook recording session metadata to .pipeline/last_handoff.md\n\
           inject-wake     SessionStart hook injecting last handoff state into conversation\n\
           inspect-state   Concise Markdown repository state summary for tools.toml\n"
    );
}
