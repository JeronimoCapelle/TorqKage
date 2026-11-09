//! Property-based (`proptest`) and snapshot (`insta`) verification suite for `tkg-common`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "Integration property and snapshot test suite"
)]

use std::path::Path;

use proptest::prelude::*;
use tkg_common::{
    AgentFrontmatter, BlockCommentDelimiters, PathPolicy, SyntaxConfig,
    check_dangerous_files_with_security,
    harness::{Antigravity, ClaudeCode, OpenCode},
    models::extract_compiler_error_signature,
    strip_comments_and_normalize_with_syntax, strip_delimited, strip_delimited_and_normalize,
    strip_tests_and_normalize_with_syntax,
};

fn rust_syntax() -> SyntaxConfig {
    SyntaxConfig {
        line_comments: vec!["//".to_owned()],
        block_comments: vec![BlockCommentDelimiters::new("/*", "*/")],
        test_attributes: vec!["test".to_owned(), "cfg(test)".to_owned()],
        test_prefixes: Vec::new(),
    }
}

fn python_syntax() -> SyntaxConfig {
    SyntaxConfig {
        line_comments: vec!["#".to_owned()],
        block_comments: vec![
            BlockCommentDelimiters::new("\"\"\"", "\"\"\""),
            BlockCommentDelimiters::new("'''", "'''"),
        ],
        test_attributes: vec!["@pytest".to_owned(), "@unittest".to_owned()],
        test_prefixes: vec!["test_".to_owned(), "Test".to_owned()],
    }
}

proptest! {
    #[test]
    fn prop_strip_comments_never_panics_and_is_idempotent(input in "\\PC*") {
        let rs_syntax = rust_syntax();
        let py_syntax = python_syntax();

        let once_rs = strip_comments_and_normalize_with_syntax(&input, &rs_syntax);
        let twice_rs = strip_comments_and_normalize_with_syntax(&once_rs, &rs_syntax);
        prop_assert_eq!(once_rs, twice_rs);

        let once_py = strip_comments_and_normalize_with_syntax(&input, &py_syntax);
        let twice_py = strip_comments_and_normalize_with_syntax(&once_py, &py_syntax);
        prop_assert_eq!(once_py, twice_py);
    }

    #[test]
    fn prop_strip_tests_never_panics_and_is_idempotent(input in "\\PC*") {
        let rs_syntax = rust_syntax();
        let py_syntax = python_syntax();

        let once_rs = strip_tests_and_normalize_with_syntax(&input, &rs_syntax, None, None);
        let twice_rs = strip_tests_and_normalize_with_syntax(&once_rs, &rs_syntax, None, None);
        prop_assert_eq!(once_rs, twice_rs);

        let once_py = strip_tests_and_normalize_with_syntax(&input, &py_syntax, None, None);
        let twice_py = strip_tests_and_normalize_with_syntax(&once_py, &py_syntax, None, None);
        prop_assert_eq!(once_py, twice_py);
    }

    #[test]
    fn prop_strip_delimited_is_idempotent(
        before in "[a-zA-Z0-9_ ]{0,40}",
        middle in "\\PC{0,60}",
        after in "[a-zA-Z0-9_ ]{0,40}",
    ) {
        let start = "<<START>>";
        let end = "<<END>>";
        let combined = format!("{before}{start}{middle}{end}{after}");
        let stripped = strip_delimited(&combined, start, end);
        let stripped_again = strip_delimited(&stripped, start, end);
        prop_assert_eq!(&stripped, &stripped_again);
        prop_assert!(!stripped.contains(start));
    }

    #[test]
    fn prop_frontmatter_roundtrip_preserves_fields(
        name in "[a-z][a-z0-9_-]{1,20}",
        description in "[a-zA-Z0-9 .,_-]{1,50}",
        effort in prop_oneof![Just("low".to_owned()), Just("medium".to_owned()), Just("high".to_owned())],
        main_agent in any::<bool>(),
        subagent in any::<bool>(),
        directives in "[a-zA-Z0-9 #._-]{1,80}",
    ) {
        let fm = AgentFrontmatter {
            name,
            description,
            effort,
            permission_mode: "default".to_owned(),
            command_execution_policy: "turbo".to_owned(),
            main_agent,
            subagent,
            inherit_mcp: false,
            allowed_subagents: vec!["helper".to_owned()],
            pre_launch_checks: vec!["workspace_clean".to_owned()],
            allowed_dirs: vec!["src".to_owned()],
            disallowed_dirs: vec!["docs".to_owned()],
            allowed_files: vec!["Cargo.toml".to_owned()],
            disallowed_files: vec!["deny.toml".to_owned()],
            path_policies: vec![PathPolicy::comments_only("src/**/*.rs")],
            native_tools: vec!["read_file".to_owned()],
            mcp_tools: vec!["modify_code".to_owned()],
            directives: format!("{directives}\n"),
        };

        let serialized = fm.serialize_for_harness(&Antigravity);
        let parsed = AgentFrontmatter::parse(&serialized).expect("valid serialized frontmatter");

        prop_assert_eq!(parsed.name, fm.name);
        prop_assert_eq!(parsed.description, fm.description);
        prop_assert_eq!(parsed.effort, fm.effort);
        prop_assert_eq!(parsed.main_agent, fm.main_agent);
        prop_assert_eq!(parsed.subagent, fm.subagent);
        prop_assert_eq!(parsed.allowed_dirs, fm.allowed_dirs);
        prop_assert_eq!(parsed.disallowed_dirs, fm.disallowed_dirs);
        prop_assert_eq!(parsed.path_policies, fm.path_policies);
        prop_assert_eq!(parsed.mcp_tools, fm.mcp_tools);
    }

    #[test]
    fn prop_arbitrary_inputs_never_panic_parsers(raw in "\\PC*") {
        drop(AgentFrontmatter::parse(&raw));
        drop(extract_compiler_error_signature(&raw, None));
        drop(check_dangerous_files_with_security(Path::new(&raw), None));
        drop(strip_delimited_and_normalize(&raw, "/*", "*/"));
    }
}

#[test]
fn test_snapshot_frontmatter_dialects() {
    let fm = AgentFrontmatter {
        name: "test-author".to_owned(),
        description: "Design Verification & Formal Test Engineer".to_owned(),
        effort: "high".to_owned(),
        permission_mode: "default".to_owned(),
        command_execution_policy: "turbo".to_owned(),
        main_agent: true,
        subagent: true,
        inherit_mcp: false,
        allowed_subagents: vec!["mini".to_owned()],
        pre_launch_checks: vec!["workspace_clean".to_owned()],
        allowed_dirs: vec!["tests".to_owned(), "src".to_owned()],
        disallowed_dirs: vec!["docs".to_owned()],
        allowed_files: Vec::new(),
        disallowed_files: vec!["Cargo.toml".to_owned()],
        path_policies: vec![PathPolicy::tests_only("src/**")],
        native_tools: vec!["read_file".to_owned()],
        mcp_tools: vec!["cargo_nextest".to_owned(), "modify_code".to_owned()],
        directives: "# Verification Directives\nWrite deterministic property tests.\n".to_owned(),
    };

    insta::assert_snapshot!(
        "frontmatter_antigravity",
        fm.serialize_for_harness(&Antigravity)
    );
    insta::assert_snapshot!("frontmatter_opencode", fm.serialize_for_harness(&OpenCode));
    insta::assert_snapshot!(
        "frontmatter_claudecode",
        fm.serialize_for_harness(&ClaudeCode)
    );
}
