//! Agent scoping and tool allowlisting for `agy-mcp`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves the requested agent from command line arguments, environment variables, or parent process cmdline.
#[must_use]
pub(crate) fn resolve_agent_scope(args: &[String]) -> Option<String> {
    // 1. Check CLI args
    let mut i = 1;
    while i < args.len() {
        if let Some(arg) = args.get(i)
            && (arg == "--agent" || arg == "-a" || arg == "--scope" || arg == "-s")
            && let Some(val) = args.get(i.saturating_add(1))
        {
            return Some(val.trim().to_owned());
        }
        i = i.saturating_add(1);
    }

    // 2. Check environment variables
    if let Ok(val) = std::env::var("AGY_AGENT")
        && !val.trim().is_empty()
    {
        return Some(val.trim().to_owned());
    }
    if let Ok(val) = std::env::var("MCP_SCOPE")
        && !val.trim().is_empty()
    {
        return Some(val.trim().to_owned());
    }

    // 3. Inspect parent process cmdline on Linux (/proc/$PPID/cmdline)
    if let Some(agent) = detect_agent_from_parent_process() {
        return Some(agent);
    }

    None
}

/// Detects `--agent <name>` from the parent process command line if running under `agy`.
fn detect_agent_from_parent_process() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let ppid = std::os::unix::process::parent_id();
        let cmdline_path = format!("/proc/{ppid}/cmdline");
        if let Ok(bytes) = fs::read(cmdline_path) {
            let cmd_parts: Vec<&str> = bytes
                .split(|&b| b == 0)
                .filter_map(|s| std::str::from_utf8(s).ok())
                .collect();

            let is_agy = cmd_parts.first().is_some_and(|exe| exe.contains("agy"));

            let mut i = 0;
            while i < cmd_parts.len() {
                if let Some(&arg) = cmd_parts.get(i) {
                    if let Some(val) = arg.strip_prefix("--agent=") {
                        return Some(val.trim().to_owned());
                    }
                    if (arg == "--agent" || arg == "-a")
                        && let Some(&val) = cmd_parts.get(i.saturating_add(1))
                        && !val.starts_with('-')
                    {
                        return Some(val.trim().to_owned());
                    }
                }
                i = i.saturating_add(1);
            }

            if is_agy {
                return Some("copilot".to_owned());
            }
        }
    }
    None
}

/// Resolves the list of allowed MCP tools for the given agent name.
/// Checks the agent markdown file frontmatter first, then falls back to canonical defaults.
#[must_use]
pub(crate) fn resolve_allowed_tools(workspace_root: &Path, agent_name: &str) -> HashSet<String> {
    if let Some(tools) = parse_tools_from_agent_files(workspace_root, agent_name)
        && !tools.is_empty()
    {
        return tools;
    }

    canonical_agent_tools(agent_name)
}

fn parse_tools_from_agent_files(workspace_root: &Path, agent_name: &str) -> Option<HashSet<String>> {
    let mut candidates = vec![
        workspace_root.join(format!("plugins/agy-suite/agents/{agent_name}.md")),
        workspace_root.join(format!(".agents/agents/{agent_name}.md")),
    ];

    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(home);
        candidates.push(home_path.join(format!(".gemini/config/plugins/agy-suite/agents/{agent_name}.md")));
        candidates.push(home_path.join(format!(".gemini/config/agents/{agent_name}.md")));
    }

    for path in candidates {
        if path.is_file()
            && let Ok(content) = fs::read_to_string(&path)
            && let Some(tools) = extract_mcp_tools_from_markdown(&content)
        {
            return Some(tools);
        }
    }

    None
}

/// Extracts tools listed under `mcp_tools:` or `mcpTools:` in the YAML frontmatter.
#[must_use]
pub(crate) fn extract_mcp_tools_from_markdown(content: &str) -> Option<HashSet<String>> {
    let trimmed = content.trim_start();
    let rest = trimmed.strip_prefix("---")?;

    // Find the end of frontmatter
    let end_idx = rest.find("\n---")?;
    let frontmatter = rest.get(..end_idx)?;

    let mut in_mcp_tools_block = false;
    let mut tools = HashSet::new();

    for line in frontmatter.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.starts_with("mcp_tools:") || trimmed_line.starts_with("mcpTools:") {
            in_mcp_tools_block = true;
            continue;
        }

        if in_mcp_tools_block {
            if let Some(tool_part) = trimmed_line.strip_prefix('-') {
                let tool_name = tool_part.trim().trim_matches(['"', '\'']);
                if !tool_name.is_empty() {
                    tools.insert(tool_name.to_owned());
                }
            } else if !trimmed_line.is_empty() && !trimmed_line.starts_with('#') {
                // Encountered another key
                break;
            }
        }
    }

    if tools.is_empty() {
        None
    } else {
        Some(tools)
    }
}

#[allow(clippy::too_many_lines, reason = "Canonical default mapping table for all 9 agents")]
fn canonical_agent_tools(agent: &str) -> HashSet<String> {
    let list: &[&str] = match agent {
        "coder" => &[
            "read_project_file",
            "modify_source_code",
            "write_source_file",
            "search_codebase",
            "inspect_workspace_diff",
            "run_quick_check",
            "run_cargo_verification",
            "run_cargo_test",
            "run_cargo_clippy",
            "run_cargo_clippy_fix",
            "run_cargo_fmt",
            "query_tool_help",
        ],
        "copilot" => &[
            "read_project_file",
            "write_project_file",
            "modify_project_code",
            "search_codebase",
            "inspect_module_structure",
            "inspect_project_state",
            "inspect_workspace_diff",
            "inspect_parent_commit",
            "inspect_revision_log",
            "inspect_bookmarks",
            "run_quick_check",
            "query_tool_help",
        ],
        "finisher" => &[
            "read_project_file",
            "search_codebase",
            "query_project_spec",
            "inspect_module_structure",
            "inspect_workspace_diff",
            "inspect_parent_commit",
            "inspect_revision_log",
            "inspect_workspace_conflicts",
            "inspect_bookmarks",
            "inspect_project_state",
            "run_cargo_nextest",
            "run_cargo_clippy",
            "run_cargo_doc",
            "run_cargo_fmt",
            "run_cargo_check_targets",
            "run_git_cliff",
            "query_tool_help",
        ],
        "mini" => &[
            "eval_symbolic_math",
        ],
        "perf-analyst" => &[
            "read_project_file",
            "search_codebase",
            "run_cargo_asm",
            "run_cargo_bench",
            "run_gungraun_bench",
            "run_criterion_bench",
            "run_divan_bench",
            "run_cargo_careful",
            "run_cargo_miri",
            "run_flamegraph",
            "run_samply_profile",
            "run_cargo_perf",
            "run_cargo_bloat",
            "run_hyperfine",
            "run_valgrind_cachegrind",
            "run_strace_summary",
            "run_cargo_zigbuild",
            "run_nasm_check",
            "run_qemu_kernel_test",
            "analyze_llvm_mca",
            "inspect_binary_sections",
            "inspect_symbol_table",
            "write_design_doc",
            "query_tool_help",
        ],
        "planner" => &[
            "read_project_file",
            "write_design_doc",
            "modify_design_doc",
            "search_codebase",
            "inspect_module_structure",
            "query_project_spec",
            "query_tool_help",
        ],
        "spec-checker" => &[
            "read_project_file",
            "search_codebase",
            "query_project_spec",
            "inspect_module_structure",
            "inspect_bookmarks",
            "inspect_revision_log",
            "run_cargo_clippy",
            "run_cargo_deny",
            "run_cargo_machete",
            "run_cargo_semver_checks",
            "run_cargo_public_api",
            "run_cargo_hack",
            "run_cargo_outdated",
            "run_ast_grep",
            "query_tool_help",
        ],
        "sysadmin" => &[
            "query_tool_help",
        ],
        "test-author" => &[
            "read_project_file",
            "search_codebase",
            "write_test_file",
            "modify_test_code",
            "run_quick_check",
            "run_cargo_verification",
            "run_cargo_test",
            "run_cargo_nextest",
            "run_cargo_check_targets",
            "run_coverage_summary",
            "run_mutation_tests",
            "run_cargo_insta",
            "run_cargo_fuzz",
            "run_property_tests",
            "run_concurrency_tests",
            "query_tool_help",
        ],
        _ => &[],
    };

    list.iter().map(|&s| s.to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mcp_tools_from_markdown() {
        let md = "---\nname: coder\ntools: []\nmcp_tools:\n  - read_project_file\n  - modify_source_code\n  - write_source_file\n---\nSome body\n";
        let tools = extract_mcp_tools_from_markdown(md).unwrap();
        assert_eq!(tools.len(), 3);
        assert!(tools.contains("read_project_file"));
        assert!(tools.contains("modify_source_code"));
        assert!(tools.contains("write_source_file"));
    }

    #[test]
    fn test_canonical_agent_tools() {
        let coder_tools = canonical_agent_tools("coder");
        assert!(coder_tools.contains("modify_source_code"));
        assert!(!coder_tools.contains("modify_design_doc"));

        let mini_tools = canonical_agent_tools("mini");
        assert_eq!(mini_tools.len(), 1);
        assert!(mini_tools.contains("eval_symbolic_math"));
    }
}
