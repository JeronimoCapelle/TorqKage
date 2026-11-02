//! `cli_manual` action handler providing in-session help, tldr, navi recipes, and man snippets.

use std::process::Command;
use crate::guards::truncate_output;
use crate::models::ToolDefinition;

/// Executes a CLI manual lookup action (`query_tool_help`).
///
/// Priority ladder:
/// 1. `navi` cheatsheet recipe (if `query` parameter provided)
/// 2. `tldr` cheatsheet (concise real-world examples)
/// 3. `--help` CLI output (short flag reference, capped at 60 lines)
/// 4. `man` synopsis snippet (NAME + SYNOPSIS block for Linux system binaries)
/// 5. Default workspace tool catalog
pub(crate) fn execute(
    tool: &ToolDefinition,
    arguments: &serde_json::Value,
) -> (String, bool) {
    let tool_name = arguments
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim);

    let query = arguments
        .get("query")
        .and_then(serde_json::Value::as_str)
        .map(str::trim);

    let content = match tool_name {
        Some(name) if !name.is_empty() => resolve_tool_help(name, query),
        _ => default_tool_catalog(),
    };

    let truncated = truncate_output(
        &content,
        tool.truncate_lines.or(Some(80)),
        tool.truncate_bytes,
        tool.emit_truncation_notice,
    );

    (truncated, false)
}

fn resolve_tool_help(name: &str, query_opt: Option<&str>) -> String {
    // 1. If a specific query/topic was provided, check navi first
    if let Some(query) = query_opt
        && !query.is_empty()
        && let Some(recipe) = query_navi(name, query)
    {
        return format!("# navi cheatsheet recipe: {name} ({query})\n\n```bash\n{recipe}\n```");
    }

    // 2. Query tldr for concise, task-oriented examples
    if let Some(tldr) = query_tldr(name) {
        return format!("# tldr cheatsheet: {name}\n\n{tldr}");
    }

    // 3. Query navi with tool name alone if tldr was absent
    if let Some(recipe) = query_navi(name, "") {
        return format!("# navi cheatsheet recipe: {name}\n\n```bash\n{recipe}\n```");
    }

    // 4. Query tool's native `--help` flag
    if let Some(help) = query_cli_help(name) {
        return format!("# {name} --help\n\n{help}");
    }

    // 5. Query man page and extract concise NAME + SYNOPSIS block
    if let Some(man_synopsis) = query_man_synopsis(name) {
        return format!("# man synopsis: {name}\n\n{man_synopsis}");
    }

    format!("No manual, tldr, navi recipe, or --help usage output available for command '{name}' on host system.")
}

fn query_navi(name: &str, query: &str) -> Option<String> {
    let search_term = if query.is_empty() {
        name.to_owned()
    } else {
        format!("{name} {query}")
    };

    let output = Command::new("navi")
        .args(["--query", &search_term, "--best-match", "--print"])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn query_tldr(name: &str) -> Option<String> {
    let output = Command::new("tldr").arg(name).output().ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !text.is_empty() && !text.contains("not found in cache") {
            return Some(text);
        }
    }
    None
}

fn query_cli_help(name: &str) -> Option<String> {
    let output = Command::new(name).arg("--help").output().ok()?;

    if output.status.success() || !output.stdout.is_empty() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !text.is_empty() {
            // Cap raw --help at first 50 lines to keep token budget frugal
            let sample: Vec<&str> = text.lines().take(50).collect();
            return Some(sample.join("\n"));
        }
    }
    None
}

fn query_man_synopsis(name: &str) -> Option<String> {
    let output = Command::new("man")
        .env("MANPAGER", "cat")
        .arg(name)
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        return extract_man_snippet(&text);
    }
    None
}

fn extract_man_snippet(man_text: &str) -> Option<String> {
    let mut snippet_lines = Vec::new();
    let mut count: usize = 0;
    let mut started = false;

    for line in man_text.lines() {
        let trimmed = line.trim();

        // Stop if we hit the exhaustive OPTIONS block or exceed 35 lines
        if started && (trimmed == "OPTIONS" || count >= 35) {
            break;
        }

        if started {
            snippet_lines.push(line);
            count = count.saturating_add(1);
        } else if !trimmed.is_empty() {
            started = true;
            snippet_lines.push(line);
            count = count.saturating_add(1);
        }
    }

    if snippet_lines.is_empty() {
        None
    } else {
        Some(snippet_lines.join("\n"))
    }
}

fn default_tool_catalog() -> String {
    "# Available CLI Tooling in agy-suite\n\
     \n\
     ## Category C: Test Synthesis & Verification (Agent: test-author)\n\
     - cargo-nextest: Parallel test harness runner (3x faster than standard test runner)\n\
     - cargo-llvm-cov: Source-based branch and line code coverage reports\n\
     - cargo-mutants: Mutation testing framework uncovering weak test assertions\n\
     - cargo-insta: Snapshot testing review tool for golden files, AST, and state\n\
     - cargo-fuzz: Native libFuzzer integration for panic discovery and security bounds\n\
     \n\
     ## Category D: Performance & Hardware Diagnostics (Agent: perf-analyst)\n\
     - cargo-show-asm: Disassembles Rust symbols to assembly or LLVM-IR (SIMD/loops)\n\
     - cargo-bench: Latency and throughput microbenchmarks via Criterion/Divan\n\
     - cargo-careful: Fast runtime undefined behavior (UB) and memory sanitizer\n\
     - cargo-miri: Deep interpreter for stacked/tree borrows, UB, and pointer provenance\n\
     - cargo-flamegraph: Generates CPU callstack flamegraphs to identify bottlenecks\n\
     - cargo-perf: Linux hardware performance counters (cache misses, IPC, branch miss)\n\
     - cargo-bloat: Binary footprint and monomorphization text-size analyzer\n\
     - hyperfine: Statistical CLI benchmarking with warmup runs and cache clearing\n\
     - samply: Sampling CPU profiler with Firefox Profiler visual flamecharts\n\
     - cargo-binutils (size, nm, objdump): Low-level ELF sections, symbols, disassembly\n\
     \n\
     ## Category E: Static Analysis & SemVer (Agent: spec-checker)\n\
     - cargo-clippy: Strict idiomatic Rust linter (-D warnings)\n\
     - cargo-deny: Audits dependency tree for RUSTSEC advisories, copyleft licenses, bans\n\
     - cargo-machete: Fast scanner detecting unused dependencies in Cargo.toml\n\
     - cargo-semver-checks: Validates public API changes adhere to Semantic Versioning\n\
     - cargo-public-api: Diffs exposed public crate API signatures across git revisions\n\
     - cargo-hack: Matrix verification testing across all feature flag combinations\n\
     - cargo-modules: Module tree visualization (visibility and internal hierarchy)\n\
     - cargo-expand: Macro expansion introspection (macro_rules! and proc macros)\n\
     \n\
     ## Category B & Release (Agents: planner / finisher / operator)\n\
     - cargo-zigbuild: Cross-compiles using Zig (musl, windows, aarch64, riscv)\n\
     - git-cliff: Conventional commits changelog generator (Agent: finisher)\n\
     - cargo-add / cargo-remove / cargo-update: Managed Cargo.toml dependency changes\n\
     - cargo-generate: Scaffolds crates from git templates\n\
     - agy-hooks: Ephemeral Antigravity lifecycle hook runner\n\
     \n\
     ## Category A: Strictly USER-ONLY Actions\n\
     - cargo publish, cargo yank, cargo clean-all, cargo login: Banned from all autonomous agents\n"
        .to_owned()
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_default_catalog_returned_when_empty() {
        let tool = ToolDefinition::default();
        let args = serde_json::json!({});
        let (out, is_err) = execute(&tool, &args);
        assert!(!is_err);
        assert!(out.contains("Available CLI Tooling"));
    }

    #[test]
    fn test_extract_man_snippet() {
        let sample = "PERF(1) Manual\n\nNAME\n    perf - analysis\n\nSYNOPSIS\n    perf [OPTIONS]\n\nOPTIONS\n    -h, --help";
        let snippet = extract_man_snippet(sample).expect("snippet extraction failed");
        assert!(snippet.contains("NAME"));
        assert!(snippet.contains("SYNOPSIS"));
        assert!(!snippet.contains("-h, --help"));
    }
}
