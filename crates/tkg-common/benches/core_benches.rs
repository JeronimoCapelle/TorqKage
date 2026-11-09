//! Microbenchmark suite for `tkg-common` hot paths (`content_filter`, `frontmatter`, `tools`, `project`).

use std::path::Path;

use divan::black_box;
use tkg_common::{
    AgentFrontmatter, BlockCommentDelimiters, SyntaxConfig, check_dangerous_files_with_security,
    harness::{Antigravity, OpenCode},
    path_matches_pattern, strip_comments_and_normalize_with_syntax, strip_delimited_and_normalize,
    strip_tests_and_normalize_with_syntax,
    tools::{RawToolsConfig, resolve},
};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

const SAMPLE_RUST_SOURCE: &str = r##"
//! Module-level documentation comment.
use std::collections::HashMap;

/// Computes a bounded hash of the input slice.
pub fn compute_hash(input: &str) -> u64 {
    // Line comment explaining the raw string literal below
    let prefix = r#"raw_string_with_//_and_/*_fake_comment_*/"#;
    /* Outer block comment
       /* Nested block comment */
       End of outer block comment */
    let mut acc = 0_u64;
    for b in input.bytes().chain(prefix.bytes()) {
        acc = acc.wrapping_mul(31).wrapping_add(u64::from(b));
    }
    acc
}

// BEGIN_GENERATED_SECTION
pub const GENERATED_VERSION: u32 = 42;
// END_GENERATED_SECTION

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash_deterministic() {
        assert_eq!(compute_hash("abc"), compute_hash("abc"));
    }
}
"##;

const SAMPLE_AGENT_MANIFEST: &str = r#"---
name: coder
description: "Primary systems implementation agent"
effort: high
permissionMode: default
commandExecutionPolicy: turbo
mainAgent: true
subagent: true
inheritMcp: false
allowed_subagents:
  - test-author
  - doc-author
pre_launch_checks:
  - workspace_clean
allowed_dirs:
  - src
  - crates
disallowed_dirs:
  - docs
allowed_files:
  - Cargo.toml
disallowed_files:
  - deny.toml
path_policies:
  - path: "src/**/*.rs"
    filter: comments_only
tools:
  - read_file
mcp_tools:
  - modify_code
  - cargo_check
---
# Agent Directives
Follow strict safety and zero-warning invariants.
"#;

const SAMPLE_TOOLS_TOML: &str = r#"
[categories.compilation]
category = "compilation"
tracks_circuit_breaker = true
require_gates = ["vcs_conflicts"]
timeout_seconds = 120

[base_tools.cargo_base]
description = "Base execution template for cargo commands"
action = "static_command"
category = "compilation"

[tools.cargo_check]
inherit = "base_tools.cargo_base"
description = "Runs cargo check"
action = "parameterized_command"
command = ["cargo", "check"]
args = [
    { name = "package", required = false, flag = "-p", regex = "^[a-zA-Z0-9_-]+$", description = "Package" }
]
"#;

fn rust_syntax() -> SyntaxConfig {
    SyntaxConfig {
        line_comments: vec!["//".to_owned()],
        block_comments: vec![BlockCommentDelimiters::new("/*", "*/")],
        test_attributes: vec!["test".to_owned(), "cfg(test)".to_owned()],
        test_prefixes: Vec::new(),
    }
}

#[divan::bench]
fn bench_strip_comments_rust() -> String {
    let syntax = rust_syntax();
    strip_comments_and_normalize_with_syntax(black_box(SAMPLE_RUST_SOURCE), black_box(&syntax))
}

#[divan::bench]
fn bench_strip_tests_rust() -> String {
    let syntax = rust_syntax();
    strip_tests_and_normalize_with_syntax(
        black_box(SAMPLE_RUST_SOURCE),
        black_box(&syntax),
        None,
        None,
    )
}

#[divan::bench]
fn bench_strip_delimited() -> String {
    strip_delimited_and_normalize(
        black_box(SAMPLE_RUST_SOURCE),
        black_box("// BEGIN_GENERATED_SECTION"),
        black_box("// END_GENERATED_SECTION"),
    )
}

#[divan::bench]
fn bench_frontmatter_parse_and_serialize() -> String {
    let fm = AgentFrontmatter::parse(black_box(SAMPLE_AGENT_MANIFEST)).unwrap_or_default();
    let agy = Antigravity;
    let oc = OpenCode;
    drop(fm.serialize_for_harness(black_box(&oc)));
    fm.serialize_for_harness(black_box(&agy))
}

#[divan::bench]
fn bench_resolve_full_tools_toml() -> usize {
    let raw = RawToolsConfig::from_toml_str(black_box(SAMPLE_TOOLS_TOML));
    resolve(black_box(&raw)).len()
}

#[divan::bench]
fn bench_path_glob_and_security_guards() -> bool {
    let p = Path::new("crates/tkg-common/src/content_filter.rs");
    drop(check_dangerous_files_with_security(black_box(p), None));
    path_matches_pattern(
        black_box("crates/tkg-common/src/content_filter.rs"),
        black_box("crates/**/*.rs"),
    )
}
