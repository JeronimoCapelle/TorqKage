//! # agy-mcp
//!
//! Declarative, bounded action Model Context Protocol (MCP) server daemon for Antigravity over stdio.

#![allow(clippy::print_stderr, reason = "Daemon diagnostics are logged to stderr")]

mod actions;
mod config;
mod guards;
mod models;
mod protocol;
mod schema;
mod server;
mod validator;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use agy_common::WorkspaceResolver;
use server::McpServer;

#[tokio::main]
async fn main() -> ExitCode {
    let (workspace_root, config_path) = resolve_workspace_and_config();

    eprintln!("[agy-mcp] Workspace root: '{}'", workspace_root.display());
    eprintln!("[agy-mcp] Loading declarative tools configuration from '{}'...", config_path.display());

    let tools = match config::load_tools_from_path(&config_path) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("🛑 [agy-mcp] Startup failure: {err}");
            return ExitCode::FAILURE;
        }
    };

    let report = validator::validate_tools(&tools);
    eprintln!("{}", report.summary());

    for warning in report.warnings {
        eprintln!("[agy-mcp] Warning: {warning}");
    }

    let server = McpServer::new(tools, workspace_root);

    if let Err(err) = server.run_stdio().await {
        eprintln!("🛑 [agy-mcp] Stdio loop error: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn resolve_workspace_and_config() -> (PathBuf, PathBuf) {
    let args: Vec<String> = std::env::args().collect();
    let mut custom_config = None;
    let mut custom_workspace = None;

    let mut i = 1;
    while i < args.len() {
        if let Some(arg) = args.get(i) {
            if (arg == "--config" || arg == "-c")
                && let Some(val) = args.get(i.saturating_add(1))
            {
                custom_config = Some(PathBuf::from(val));
                i = i.saturating_add(1);
            } else if (arg == "--workspace" || arg == "-w")
                && let Some(val) = args.get(i.saturating_add(1))
            {
                custom_workspace = Some(PathBuf::from(val));
                i = i.saturating_add(1);
            }
        }
        i = i.saturating_add(1);
    }

    let config_path = custom_config.unwrap_or_else(|| {
        let ws = custom_workspace.clone().unwrap_or_else(|| {
            WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."))
        });
        ws.join("tools.toml")
    });

    let workspace_root = custom_workspace.unwrap_or_else(|| {
        if let Some(parent) = config_path.parent()
            && parent != Path::new("")
        {
            parent.to_path_buf()
        } else {
            WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."))
        }
    });

    (workspace_root, config_path)
}
