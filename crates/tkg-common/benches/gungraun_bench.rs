//! Deterministic Valgrind Callgrind instruction-count benchmarks (`gungraun`).

#![allow(
    unused_qualifications,
    reason = "gungraun #[library_benchmark] macro expansion emits fully qualified paths"
)]

use std::hint::black_box;

use gungraun::{library_benchmark, library_benchmark_group, main};
use tkg_common::{
    AgentFrontmatter, BlockCommentDelimiters, SyntaxConfig,
    strip_comments_and_normalize_with_syntax, strip_delimited_and_normalize,
    strip_tests_and_normalize_with_syntax,
};

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

fn rust_syntax() -> SyntaxConfig {
    SyntaxConfig {
        line_comments: vec!["//".to_owned()],
        block_comments: vec![BlockCommentDelimiters::new("/*", "*/")],
        test_attributes: vec!["test".to_owned(), "cfg(test)".to_owned()],
        test_prefixes: Vec::new(),
    }
}

#[library_benchmark]
fn callgrind_strip_comments() -> String {
    let syntax = rust_syntax();
    strip_comments_and_normalize_with_syntax(black_box(SAMPLE_RUST_SOURCE), black_box(&syntax))
}

#[library_benchmark]
fn callgrind_strip_tests() -> String {
    let syntax = rust_syntax();
    strip_tests_and_normalize_with_syntax(
        black_box(SAMPLE_RUST_SOURCE),
        black_box(&syntax),
        None,
        None,
    )
}

#[library_benchmark]
fn callgrind_strip_delimited() -> String {
    strip_delimited_and_normalize(
        black_box(SAMPLE_RUST_SOURCE),
        black_box("// BEGIN_GENERATED_SECTION"),
        black_box("// END_GENERATED_SECTION"),
    )
}

#[library_benchmark]
fn callgrind_frontmatter_parse() -> Option<AgentFrontmatter> {
    AgentFrontmatter::parse(black_box(SAMPLE_AGENT_MANIFEST)).ok()
}

library_benchmark_group!(
    name = tkg_common_callgrind_group;
    benchmarks =
        callgrind_strip_comments,
        callgrind_strip_tests,
        callgrind_strip_delimited,
        callgrind_frontmatter_parse
);

main!(library_benchmark_groups = tkg_common_callgrind_group);
