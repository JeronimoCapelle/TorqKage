//! Headless CLI summary, status printer, and offline validation reports.

use std::path::Path;
use crate::model::{AgentManifest, ToolItem};
use crate::vcs::run_plugin_validation;

/// Prints an operational status summary of the agent fleet and tool assignments.
pub(crate) fn print_fleet_status(agents: &[AgentManifest], tools: &[ToolItem]) {
    println!("================================================================================");
    println!("                   AGY-CTRL: AGENT FLEET & CAPABILITY STATUS                   ");
    println!("================================================================================");
    println!("Total Registered Agents: {:<2} | Total Declared MCP Tools: {:<2}", agents.len(), tools.len());
    println!("--------------------------------------------------------------------------------");
    println!("{:<14} | {:<7} | {:<12} | {:<10} | {:<12}", "Agent Name", "Effort", "Permissions", "Tool Count", "Mode");
    println!("---------------+---------+--------------+------------+--------------------------");

    for agent in agents {
        let tool_count = format!("{} tools", agent.mcp_tools.len());
        println!(
            "{:<14} | {:<7} | {:<12} | {:<10} | {}",
            agent.name,
            agent.effort,
            agent.permission_mode,
            tool_count,
            if agent.inherit_mcp { "Ambient (UNSAFE)" } else { "Strictly Scoped" }
        );
    }

    println!("--------------------------------------------------------------------------------");
    println!("\nDetailed MCP Tool Distribution Matrix:");
    println!("--------------------------------------------------------------------------------");

    for agent in agents {
        println!("\n▶ [{}] ({} tools):", agent.name, agent.mcp_tools.len());
        if agent.mcp_tools.is_empty() {
            println!("    (No MCP tools assigned - Air-Gapped baseline)");
        } else {
            let mut line = String::from("    ");
            for (idx, tool) in agent.mcp_tools.iter().enumerate() {
                line.push_str(tool);
                if idx.saturating_add(1) < agent.mcp_tools.len() {
                    line.push_str(", ");
                }
                if line.len() > 72 {
                    println!("{line}");
                    line = String::from("    ");
                }
            }
            if !line.trim().is_empty() && line != "    " {
                println!("{line}");
            }
        }
    }

    println!("\n================================================================================");
}

/// Runs a headless pre-flight validation check and reports results.
///
/// # Errors
/// Returns an error if validation fails.
pub(crate) fn print_validation_report(workspace_root: &Path) -> Result<(), String> {
    println!("Running offline pre-flight validation for agy-suite...");
    match run_plugin_validation(workspace_root) {
        Ok(out) => {
            println!("✔ Validation Passed:\n{out}");
            Ok(())
        }
        Err(err) => {
            eprintln!("❌ Validation Failed:\n{err}");
            Err(err)
        }
    }
}
