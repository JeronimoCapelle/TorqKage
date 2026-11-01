//! # agy-mcp
//!
//! Native Model Context Protocol (MCP) server daemon for Antigravity over stdio.

#![allow(clippy::print_stderr, reason = "Daemon diagnostics are logged to stderr")]

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("agy-mcp: initialized stdio transport");
    Ok(())
}
