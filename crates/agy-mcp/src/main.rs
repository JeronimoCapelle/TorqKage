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
    let workspace_root = WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."));
    let config_path = resolve_config_path(&workspace_root);

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

fn resolve_config_path(workspace_root: &Path) -> PathBuf {
    let args: Vec<String> = std::env::args().collect();

    for i in 1..args.len() {
        if let Some(arg) = args.get(i)
            && (arg == "--config" || arg == "-c")
            && let Some(val) = args.get(i.saturating_add(1))
        {
            return PathBuf::from(val);
        }
    }

    // Default 1: current directory
    let local = Path::new("tools.toml");
    if local.exists() {
        return local.to_path_buf();
    }

    // Default 2: workspace root
    let root_config = workspace_root.join("tools.toml");
    if root_config.exists() {
        return root_config;
    }

    PathBuf::from("tools.toml")
}
