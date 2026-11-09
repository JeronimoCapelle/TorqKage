//! Standalone example demonstrating `tkg-common` workspace discovery, `project.toml`
//! inference, agent manifest resolution, and `tools.toml` inheritance resolution.

#![allow(clippy::print_stdout, reason = "Runnable CLI example output")]

use tkg_common::{
    ProjectConfig, WorkspaceResolver, discover_agent_frontmatters, tools::RawToolsConfig,
};

fn main() {
    let root = WorkspaceResolver::find_root(None)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    let project = ProjectConfig::detect_or_load(&root);
    let agents = discover_agent_frontmatters(&root);
    let tools = RawToolsConfig::load_resolved_map(&root);

    println!("Workspace Root : {}", root.display());
    println!(
        "Project        : {} (language: {}, harness: {})",
        project.project.name.as_deref().unwrap_or("unnamed"),
        project.project.language.as_deref().unwrap_or("auto"),
        project.runner.harness().id()
    );
    println!("Agents Found   : {}", agents.len());
    println!("MCP Tools      : {}", tools.len());
}
