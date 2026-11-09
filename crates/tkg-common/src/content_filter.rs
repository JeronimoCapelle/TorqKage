//! Declarative content policy engine for agent-confined file modifications.
//!
//! Enforces mathematical invariant filtering:
//! `strip_allowed_domain(original) == strip_allowed_domain(new_content)`
//!
//! Supports:
//! - `CommentsOnly`: Only docstrings, line/block comments, doctests, and whitespace may change.
//! - `TestsOnly`: Only unit test blocks, test items, and test functions may change.
//! - `Delimited`: Only content bounded between explicit `start_marker` and `end_marker` delimiters may change.
//!
//! Fully language-agnostic: syntax rules (line comments, block comments, test attributes, test prefixes)
//! can be auto-inferred by file extension or `project.language`, or declared in `project.toml`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Policy filter kind specifying the allowed modification domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFilterKind {
    /// Allows comments, docstrings, doctests, and formatting whitespace.
    #[default]
    #[serde(alias = "comments")]
    CommentsOnly,
    /// Allows unit test modules, test items, test functions, and test blocks.
    #[serde(alias = "tests")]
    TestsOnly,
    /// Allows modifications strictly delimited by configured start and end markers.
    Delimited,
}

/// Block comment delimiter pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockCommentDelimiters {
    pub start: String,
    pub end: String,
}

impl BlockCommentDelimiters {
    /// Constructs a new block comment delimiter pair.
    #[must_use]
    pub fn new(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }
}

/// Declarative language syntax rules for comments and test blocks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "RawSyntaxConfig")]
pub struct SyntaxConfig {
    /// Line comment prefixes (e.g. `["//"]` for C/Rust/Go/Zig, `["#"]` for Python/Shell, `["--"]` for Lua/SQL).
    #[serde(default)]
    pub line_comments: Vec<String>,
    /// Block comment delimiter pairs (e.g. `/* ... */`, `""" ... """`).
    #[serde(default)]
    pub block_comments: Vec<BlockCommentDelimiters>,
    /// Test attribute or decorator prefixes (e.g. `["#[test]", "#[cfg(test)]"]`, `["@pytest", "@unittest"]`, `["TEST(", "TEST_F("]`).
    #[serde(default)]
    pub test_attributes: Vec<String>,
    /// Function or item name prefixes that identify test definitions (e.g. `["test_"]` for Python, `["Test"]` for Go).
    #[serde(default)]
    pub test_prefixes: Vec<String>,
}

#[derive(Deserialize, Default)]
struct RawSyntaxConfig {
    #[serde(default)]
    line_comments: Vec<String>,
    #[serde(default)]
    line_comment: Option<String>,
    #[serde(default)]
    block_comments: Vec<BlockCommentDelimiters>,
    #[serde(default)]
    test_attributes: Vec<String>,
    #[serde(default)]
    test_attribute: Option<String>,
    #[serde(default)]
    test_prefixes: Vec<String>,
    #[serde(default)]
    test_prefix: Option<String>,
}

impl From<RawSyntaxConfig> for SyntaxConfig {
    fn from(mut raw: RawSyntaxConfig) -> Self {
        if raw.line_comments.is_empty()
            && let Some(lc) = raw.line_comment
        {
            raw.line_comments.push(lc);
        }
        if raw.test_attributes.is_empty()
            && let Some(ta) = raw.test_attribute
        {
            raw.test_attributes.push(ta);
        }
        if raw.test_prefixes.is_empty()
            && let Some(tp) = raw.test_prefix
        {
            raw.test_prefixes.push(tp);
        }
        Self {
            line_comments: raw.line_comments,
            block_comments: raw.block_comments,
            test_attributes: raw.test_attributes,
            test_prefixes: raw.test_prefixes,
        }
    }
}

impl SyntaxConfig {
    /// Generic default syntax providing standard comment syntax (`//`, `/* */`).
    /// Language-specific test attributes and syntax rules are loaded deterministically
    /// from `[policies.syntax]` in `project.toml` or agent frontmatter.
    #[must_use]
    pub fn generic() -> Self {
        Self {
            line_comments: vec!["//".to_owned()],
            block_comments: vec![BlockCommentDelimiters::new("/*", "*/")],
            test_attributes: Vec::new(),
            test_prefixes: Vec::new(),
        }
    }
}

/// Declarative path policy binding a workspace path pattern to a content filter.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "RawPathPolicy")]
pub struct PathPolicy {
    /// Relative path or directory pattern (e.g. `"src"`, `"src/lib.rs"`).
    pub path: String,
    /// The content filter kind enforced on matching files.
    pub filter: ContentFilterKind,
    /// Optional opening marker for delimited sections (e.g. `"// BEGIN TESTS"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_marker: Option<String>,
    /// Optional closing marker for delimited sections (e.g. `"// END TESTS"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_marker: Option<String>,
    /// Optional line comment overrides (e.g. `["#"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_comments: Vec<String>,
    /// Optional block comment overrides (e.g. `[ { start = "/*", end = "*/" } ]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub block_comments: Vec<BlockCommentDelimiters>,
    /// Optional test attribute overrides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_attributes: Vec<String>,
    /// Optional test prefix overrides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_prefixes: Vec<String>,
}

#[derive(Deserialize, Default)]
struct RawPathPolicy {
    #[serde(default)]
    path: String,
    #[serde(default)]
    filter: ContentFilterKind,
    #[serde(default)]
    start_marker: Option<String>,
    #[serde(default)]
    end_marker: Option<String>,
    #[serde(default)]
    line_comments: Vec<String>,
    #[serde(default)]
    line_comment: Option<String>,
    #[serde(default)]
    block_comments: Vec<BlockCommentDelimiters>,
    #[serde(default)]
    block_comment_start: Option<String>,
    #[serde(default)]
    block_comment_end: Option<String>,
    #[serde(default)]
    test_attributes: Vec<String>,
    #[serde(default)]
    test_prefixes: Vec<String>,
}

impl From<RawPathPolicy> for PathPolicy {
    fn from(mut raw: RawPathPolicy) -> Self {
        if raw.line_comments.is_empty()
            && let Some(lc) = raw.line_comment
        {
            raw.line_comments.push(lc);
        }
        if raw.block_comments.is_empty()
            && (raw.block_comment_start.is_some() || raw.block_comment_end.is_some())
        {
            raw.block_comments.push(BlockCommentDelimiters::new(
                raw.block_comment_start.unwrap_or_default(),
                raw.block_comment_end.unwrap_or_default(),
            ));
        }
        Self {
            path: raw.path,
            filter: raw.filter,
            start_marker: raw.start_marker,
            end_marker: raw.end_marker,
            line_comments: raw.line_comments,
            block_comments: raw.block_comments,
            test_attributes: raw.test_attributes,
            test_prefixes: raw.test_prefixes,
        }
    }
}

impl PathPolicy {
    /// Constructs a `CommentsOnly` policy for the specified path.
    #[must_use]
    pub fn comments_only(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            filter: ContentFilterKind::CommentsOnly,
            ..Default::default()
        }
    }

    /// Constructs a `TestsOnly` policy for the specified path.
    #[must_use]
    pub fn tests_only(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            filter: ContentFilterKind::TestsOnly,
            ..Default::default()
        }
    }

    /// Constructs a `Delimited` policy for the specified path with custom markers.
    #[must_use]
    pub fn delimited(
        path: impl Into<String>,
        start_marker: impl Into<String>,
        end_marker: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            filter: ContentFilterKind::Delimited,
            start_marker: Some(start_marker.into()),
            end_marker: Some(end_marker.into()),
            ..Default::default()
        }
    }

    /// Resolves the effective `SyntaxConfig` for this policy and candidate path.
    #[must_use]
    pub fn effective_syntax(
        &self,
        _candidate_path: &Path,
        project_syntax: Option<&SyntaxConfig>,
        _language: Option<&str>,
    ) -> SyntaxConfig {
        let mut base = project_syntax
            .cloned()
            .unwrap_or_else(SyntaxConfig::generic);

        if !self.line_comments.is_empty() {
            base.line_comments.clone_from(&self.line_comments);
        }
        if !self.block_comments.is_empty() {
            base.block_comments.clone_from(&self.block_comments);
        }
        if !self.test_attributes.is_empty() {
            base.test_attributes.clone_from(&self.test_attributes);
        }
        if !self.test_prefixes.is_empty() {
            base.test_prefixes.clone_from(&self.test_prefixes);
        }

        base
    }

    /// Verifies changes between `original` and `new_content` using default syntax.
    ///
    /// # Errors
    /// Returns an error if changes violate the allowed modification domain.
    pub fn verify(&self, original: &str, new_content: &str) -> Result<(), String> {
        let syntax = self.effective_syntax(Path::new(&self.path), None, None);
        self.verify_with_syntax(Path::new(&self.path), original, new_content, &syntax)
    }

    /// Verifies changes between `original` and `new_content` complying with explicit syntax rules.
    ///
    /// # Errors
    /// Returns an error if changes violate the allowed modification domain.
    pub fn verify_with_syntax(
        &self,
        _candidate_path: &Path,
        original: &str,
        new_content: &str,
        syntax: &SyntaxConfig,
    ) -> Result<(), String> {
        match self.filter {
            ContentFilterKind::CommentsOnly => {
                let norm_orig = strip_comments_and_normalize_with_syntax(original, syntax);
                let norm_new = strip_comments_and_normalize_with_syntax(new_content, syntax);

                if norm_orig == norm_new {
                    Ok(())
                } else if original.trim().is_empty() {
                    Err(format!(
                        "Path '{}' is restricted to comments-only. Creating non-comment code is forbidden.",
                        self.path
                    ))
                } else {
                    Err(format!(
                        "Path '{}' is restricted to comments-only. Modification of executable code tokens outside comments or docstrings is forbidden.",
                        self.path
                    ))
                }
            },
            ContentFilterKind::TestsOnly => {
                let norm_orig = strip_tests_and_normalize_with_syntax(
                    original,
                    syntax,
                    self.start_marker.as_deref(),
                    self.end_marker.as_deref(),
                );
                let norm_new = strip_tests_and_normalize_with_syntax(
                    new_content,
                    syntax,
                    self.start_marker.as_deref(),
                    self.end_marker.as_deref(),
                );

                if norm_orig == norm_new {
                    Ok(())
                } else if original.trim().is_empty() {
                    Err(format!(
                        "Path '{}' is restricted to tests-only. Introducing production code outside test blocks is forbidden.",
                        self.path
                    ))
                } else {
                    Err(format!(
                        "Path '{}' is restricted to tests-only. Modification of production code outside unit test blocks is forbidden.",
                        self.path
                    ))
                }
            },
            ContentFilterKind::Delimited => {
                let (start, end) = match (&self.start_marker, &self.end_marker) {
                    (Some(s), Some(e)) => (s.as_str(), e.as_str()),
                    (Some(s), None) => (s.as_str(), "// TKG:EDITABLE_END"),
                    (None, Some(e)) => ("// TKG:EDITABLE_START", e.as_str()),
                    (None, None) => {
                        if original.contains("// AGY:EDITABLE_START") {
                            ("// AGY:EDITABLE_START", "// AGY:EDITABLE_END")
                        } else {
                            ("// TKG:EDITABLE_START", "// TKG:EDITABLE_END")
                        }
                    },
                };

                let norm_orig = strip_delimited_and_normalize(original, start, end);
                let norm_new = strip_delimited_and_normalize(new_content, start, end);

                if norm_orig == norm_new {
                    Ok(())
                } else {
                    Err(format!(
                        "Path '{}' is restricted to delimited regions between '{start}' and '{end}'. Modification of code outside these markers is forbidden.",
                        self.path
                    ))
                }
            },
        }
    }
}

/// Normalizes whitespace runs into a single space and trims edges.
#[must_use]
pub fn normalize_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_whitespace = false;

    for c in s.chars() {
        if c.is_whitespace() {
            if !in_whitespace {
                result.push(' ');
                in_whitespace = true;
            }
        } else {
            result.push(c);
            in_whitespace = false;
        }
    }

    result.trim().to_owned()
}

/// Strips all comments from source code using default Rust syntax and normalizes whitespace.
///
/// # Examples
/// ```
/// use tkg_common::strip_comments_and_normalize;
///
/// let src = "fn add(a: i32, b: i32) -> i32 {\n    // compute sum\n    a + b /* inline */\n}";
/// assert_eq!(
///     strip_comments_and_normalize(src),
///     "fn add(a: i32, b: i32) -> i32 { a + b }"
/// );
/// ```
#[must_use]
pub fn strip_comments_and_normalize(input: &str) -> String {
    let syntax = SyntaxConfig::generic();
    strip_comments_and_normalize_with_syntax(input, &syntax)
}

/// Strips all comments using explicit language syntax and normalizes whitespace.
#[must_use]
pub fn strip_comments_and_normalize_with_syntax(input: &str, syntax: &SyntaxConfig) -> String {
    let stripped = strip_comments_with_syntax(input, syntax);
    normalize_whitespace(&stripped)
}

/// Lexer state-machine stripping comments while preserving code tokens and string contents.
#[must_use]
pub fn strip_comments_with_syntax(input: &str, syntax: &SyntaxConfig) -> String {
    let mut out = String::with_capacity(input.len());
    let len = input.len();
    let mut i = 0_usize;

    while let Some(rest) = input.get(i..) {
        if rest.is_empty() {
            break;
        }

        // 1. Raw string literal check: e.g. r"..." or r#"..."#
        if let Some((raw_lit, next_idx)) = try_consume_raw_string(input, i) {
            out.push_str(raw_lit);
            i = next_idx;
            continue;
        }

        // 2. Block comments checked before regular strings (handles Python """ before ")
        if let Some(next_idx) = try_consume_block_comment(input, i, &syntax.block_comments) {
            out.push(' ');
            i = next_idx;
            continue;
        }

        // 3. Line comments
        if let Some(next_idx) = try_consume_line_comment(input, i, &syntax.line_comments) {
            out.push(' ');
            i = next_idx;
            continue;
        }

        let Some(ch) = rest.chars().next() else {
            break;
        };

        // 4. Regular string literal: "..." or '...'
        if ch == '"' || ch == '\'' {
            i = consume_quoted_string(input, i, len, ch, &mut out);
            continue;
        }

        // 5. Normal character
        out.push(ch);
        i = i.saturating_add(ch.len_utf8());
    }

    out
}

fn try_consume_block_comment(
    input: &str,
    start: usize,
    block_comments: &[BlockCommentDelimiters],
) -> Option<usize> {
    let rest = input.get(start..)?;
    let len = input.len();

    for block in block_comments {
        let start_seq = block.start.as_str();
        let end_seq = block.end.as_str();
        if !start_seq.is_empty() && !end_seq.is_empty() && rest.starts_with(start_seq) {
            let mut i = start.saturating_add(start_seq.len());

            if start_seq == end_seq {
                if let Some(end_offset) = input.get(i..).and_then(|s| s.find(end_seq)) {
                    i = i.saturating_add(end_offset).saturating_add(end_seq.len());
                } else {
                    i = len;
                }
            } else {
                let mut depth = 1_usize;
                while i < len && depth > 0_usize {
                    let Some(sub) = input.get(i..) else {
                        break;
                    };
                    if sub.starts_with(start_seq) {
                        depth = depth.saturating_add(1_usize);
                        i = i.saturating_add(start_seq.len());
                    } else if sub.starts_with(end_seq) {
                        depth = depth.saturating_sub(1_usize);
                        i = i.saturating_add(end_seq.len());
                    } else if let Some(ch) = sub.chars().next() {
                        i = i.saturating_add(ch.len_utf8());
                    } else {
                        break;
                    }
                }
            }

            return Some(i);
        }
    }

    None
}

fn try_consume_line_comment(input: &str, start: usize, line_comments: &[String]) -> Option<usize> {
    let rest = input.get(start..)?;
    let len = input.len();

    for lc_seq in line_comments {
        if !lc_seq.is_empty() && rest.starts_with(lc_seq.as_str()) {
            let i = start.saturating_add(lc_seq.len());
            let next_i = input
                .get(i..)
                .and_then(|s| s.find('\n'))
                .map_or(len, |nl_offset| i.saturating_add(nl_offset));
            return Some(next_i);
        }
    }

    None
}

fn consume_quoted_string(
    input: &str,
    start: usize,
    len: usize,
    quote: char,
    out: &mut String,
) -> usize {
    out.push(quote);
    let mut i = start.saturating_add(quote.len_utf8());

    while i < len {
        let Some(sc) = input.get(i..).and_then(|s| s.chars().next()) else {
            break;
        };
        out.push(sc);
        i = i.saturating_add(sc.len_utf8());
        if sc == '\\' {
            if let Some(escaped) = input.get(i..).and_then(|s| s.chars().next()) {
                out.push(escaped);
                i = i.saturating_add(escaped.len_utf8());
            }
        } else if sc == quote {
            break;
        }
    }

    i
}

/// Helper attempting to parse a raw string literal `r#"..."#` starting at byte offset `start`.
fn try_consume_raw_string(input: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = start;

    if bytes.get(i).is_some_and(|&b| b == b'b' || b == b'c') {
        i = i.saturating_add(1_usize);
    }

    if bytes.get(i) != Some(&b'r') {
        return None;
    }
    i = i.saturating_add(1_usize);

    let mut hash_count = 0_usize;
    while bytes.get(i) == Some(&b'#') {
        hash_count = hash_count.saturating_add(1_usize);
        i = i.saturating_add(1_usize);
    }

    if bytes.get(i) != Some(&b'"') {
        return None;
    }
    i = i.saturating_add(1_usize);

    while i < len {
        if bytes.get(i) == Some(&b'"') {
            let mut matched_hashes = 0_usize;
            let mut check_idx = i.saturating_add(1_usize);
            while matched_hashes < hash_count && bytes.get(check_idx) == Some(&b'#') {
                matched_hashes = matched_hashes.saturating_add(1_usize);
                check_idx = check_idx.saturating_add(1_usize);
            }
            if matched_hashes == hash_count {
                return input.get(start..check_idx).map(|lit| (lit, check_idx));
            }
        }
        if let Some(ch) = input.get(i..).and_then(|s| s.chars().next()) {
            i = i.saturating_add(ch.len_utf8());
        } else {
            break;
        }
    }

    None
}

/// Strips unit test blocks using default Rust syntax and normalizes whitespace.
#[must_use]
pub fn strip_tests_and_normalize(
    input: &str,
    start_marker: Option<&str>,
    end_marker: Option<&str>,
) -> String {
    let syntax = SyntaxConfig::generic();
    strip_tests_and_normalize_with_syntax(input, &syntax, start_marker, end_marker)
}

/// Strips unit test blocks using explicit language syntax and normalizes whitespace.
#[must_use]
pub fn strip_tests_and_normalize_with_syntax(
    input: &str,
    syntax: &SyntaxConfig,
    start_marker: Option<&str>,
    end_marker: Option<&str>,
) -> String {
    // 1. Strip delimited markers if configured
    let delimited_stripped = if let (Some(start), Some(end)) = (start_marker, end_marker) {
        strip_delimited(input, start, end)
    } else {
        input.to_owned()
    };

    // 2. Strip comments first so definitions in comments are ignored
    let no_comments = strip_comments_with_syntax(&delimited_stripped, syntax);

    // 3. Check for file-level Rust inner attribute `#![cfg(test)]`
    let trimmed_no_comments = no_comments.trim_start();
    if trimmed_no_comments.starts_with("#![cfg(test")
        || trimmed_no_comments.starts_with("#![cfg(all(test")
    {
        return String::new();
    }

    // 4. If indentation-based (e.g. Python), strip via indentation hierarchy
    let is_indentation_based = syntax.line_comments.iter().any(|c| c == "#")
        && (syntax
            .test_prefixes
            .iter()
            .any(|p| p.starts_with("test_") || p.starts_with("Test"))
            || syntax.test_attributes.iter().any(|a| a.starts_with('@')));

    if is_indentation_based {
        let stripped_python = strip_indentation_test_items(&no_comments, syntax);
        return normalize_whitespace(&stripped_python);
    }

    // 5. Brace-based test item stripper (Rust, C/C++, Zig, Go, Java, JS/TS)
    let tests_stripped = strip_brace_test_items(&no_comments, syntax);
    normalize_whitespace(&tests_stripped)
}

/// Strips indentation-based test functions and classes (e.g. Python `def test_...`, `@pytest`).
fn strip_indentation_test_items(input: &str, syntax: &SyntaxConfig) -> String {
    let mut out = String::with_capacity(input.len());
    let mut skipping_indent: Option<usize> = None;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if skipping_indent.is_none() {
                out.push('\n');
            }
            continue;
        }

        let indent = line.len().saturating_sub(line.trim_start().len());

        if let Some(skip_lvl) = skipping_indent {
            if indent > skip_lvl {
                continue;
            }
            skipping_indent = None;
        }

        let is_test_attr = syntax
            .test_attributes
            .iter()
            .any(|a| trimmed.starts_with(a));
        let is_test_fn = trimmed.starts_with("def ")
            && syntax.test_prefixes.iter().any(|p| {
                trimmed
                    .strip_prefix("def ")
                    .unwrap_or("")
                    .trim_start()
                    .starts_with(p)
            });
        let is_test_class = trimmed.starts_with("class ")
            && syntax.test_prefixes.iter().any(|p| {
                trimmed
                    .strip_prefix("class ")
                    .unwrap_or("")
                    .trim_start()
                    .starts_with(p)
            });

        if is_test_attr || is_test_fn || is_test_class {
            skipping_indent = Some(indent);
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    out
}

/// Strips all content strictly enclosed between `start_marker` and `end_marker`.
///
/// # Examples
/// ```
/// use tkg_common::strip_delimited;
///
/// let src = "keep_before /*BEGIN*/mutable_zone/*END*/ keep_after";
/// assert_eq!(strip_delimited(src, "/*BEGIN*/", "/*END*/"), "keep_before  keep_after");
/// ```
#[must_use]
pub fn strip_delimited(input: &str, start_marker: &str, end_marker: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some((before, after_start)) = rest.split_once(start_marker) {
        out.push_str(before);
        if let Some((_, after_end)) = after_start.split_once(end_marker) {
            rest = after_end;
        } else {
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Strips delimited sections and normalizes whitespace.
#[must_use]
pub fn strip_delimited_and_normalize(input: &str, start_marker: &str, end_marker: &str) -> String {
    let stripped = strip_delimited(input, start_marker, end_marker);
    normalize_whitespace(&stripped)
}

/// Checks whether an attribute or item matches declared test attributes or test prefixes.
fn is_matching_test_item(token: &str, syntax: &SyntaxConfig) -> bool {
    let clean = token.trim();
    for attr in &syntax.test_attributes {
        if clean == attr
            || clean.ends_with(&format!("::{attr}"))
            || clean.starts_with(attr)
            || (clean.starts_with("cfg(") && clean.contains(attr))
        {
            return true;
        }
    }
    for prefix in &syntax.test_prefixes {
        if clean.starts_with(prefix) {
            return true;
        }
    }
    false
}

/// State-machine lexer that identifies test attributes/definitions and consumes the entire item.
fn parse_rust_attribute<'a>(
    input: &'a str,
    i: &mut usize,
    syntax: &SyntaxConfig,
) -> Option<(&'a str, bool)> {
    let len = input.len();
    if !input.get(*i..).is_some_and(|s| s.starts_with("#[")) {
        return None;
    }

    let start = *i;
    *i = i.saturating_add(2_usize);
    let content_start = *i;
    let mut content_end = *i;
    let mut bracket_depth = 1_usize;

    while *i < len && bracket_depth > 0_usize {
        let Some(ch) = input.get(*i..).and_then(|s| s.chars().next()) else {
            break;
        };
        if ch == '[' {
            bracket_depth = bracket_depth.saturating_add(1_usize);
            *i = i.saturating_add(1_usize);
            content_end = *i;
        } else if ch == ']' {
            bracket_depth = bracket_depth.saturating_sub(1_usize);
            if bracket_depth > 0_usize {
                content_end = i.saturating_add(1_usize);
            }
            *i = i.saturating_add(1_usize);
        } else if ch == '"' {
            *i = i.saturating_add(1_usize);
            while *i < len {
                let Some(sc) = input.get(*i..).and_then(|s| s.chars().next()) else {
                    break;
                };
                *i = i.saturating_add(sc.len_utf8());
                if sc == '\\' {
                    if let Some(esc) = input.get(*i..).and_then(|s| s.chars().next()) {
                        *i = i.saturating_add(esc.len_utf8());
                    }
                } else if sc == '"' {
                    break;
                }
            }
            content_end = *i;
        } else {
            *i = i.saturating_add(ch.len_utf8());
            content_end = *i;
        }
    }

    let full_attr = input.get(start..*i)?;
    let attr_content = input.get(content_start..content_end)?;
    let is_test = is_matching_test_item(attr_content, syntax);
    Some((full_attr, is_test))
}

fn matches_direct_test_definition(input: &str, i: usize, syntax: &SyntaxConfig) -> bool {
    let Some(rest) = input.get(i..) else {
        return false;
    };
    syntax
        .test_attributes
        .iter()
        .any(|a| rest.starts_with(a.as_str()))
        || syntax.test_prefixes.iter().any(|p| {
            rest.starts_with(p.as_str())
                || rest
                    .strip_prefix("func ")
                    .is_some_and(|after_func| after_func.starts_with(p.as_str()))
        })
}

fn skip_test_item_body(input: &str, i: &mut usize) {
    let len = input.len();
    let mut brace_depth = 0_usize;
    let mut seen_brace = false;

    while *i < len {
        if let Some((_, next_idx)) = try_consume_raw_string(input, *i) {
            *i = next_idx;
            continue;
        }

        let Some(ch) = input.get(*i..).and_then(|s| s.chars().next()) else {
            break;
        };

        if ch == '"' {
            *i = i.saturating_add(1_usize);
            while *i < len {
                let Some(sc) = input.get(*i..).and_then(|s| s.chars().next()) else {
                    break;
                };
                *i = i.saturating_add(sc.len_utf8());
                if sc == '\\' {
                    if let Some(esc) = input.get(*i..).and_then(|s| s.chars().next()) {
                        *i = i.saturating_add(esc.len_utf8());
                    }
                } else if sc == '"' {
                    break;
                }
            }
            continue;
        }

        if ch == '\''
            && let Some(sub) = input.get(*i..)
        {
            let mut chars = sub.chars();
            _ = chars.next();
            if let (Some(mid), Some('\'')) = (chars.next(), chars.next()) {
                *i = i.saturating_add(2_usize).saturating_add(mid.len_utf8());
                continue;
            }
        }

        if ch == '{' {
            seen_brace = true;
            brace_depth = brace_depth.saturating_add(1_usize);
            *i = i.saturating_add(1_usize);
        } else if ch == '}' {
            if brace_depth > 0_usize {
                brace_depth = brace_depth.saturating_sub(1_usize);
                *i = i.saturating_add(1_usize);
                if brace_depth == 0_usize {
                    break;
                }
            } else {
                *i = i.saturating_add(1_usize);
                break;
            }
        } else if ch == ';' && !seen_brace {
            *i = i.saturating_add(1_usize);
            break;
        } else {
            *i = i.saturating_add(ch.len_utf8());
        }
    }
}

fn strip_brace_test_items(input: &str, syntax: &SyntaxConfig) -> String {
    let mut out = String::with_capacity(input.len());
    let len = input.len();
    let mut i = 0_usize;

    let mut attr_buffer: Vec<(&str, bool)> = Vec::new();

    while i < len {
        let Some(ch) = input.get(i..).and_then(|s| s.chars().next()) else {
            break;
        };

        if ch.is_whitespace() {
            if attr_buffer.is_empty() {
                out.push(ch);
            }
            i = i.saturating_add(ch.len_utf8());
            continue;
        }

        // 1. Rust attribute detection: #[ ... ]
        if let Some((full_attr, is_test)) = parse_rust_attribute(input, &mut i, syntax) {
            attr_buffer.push((full_attr, is_test));
            continue;
        }

        // 2. Direct test definitions (e.g. Zig `test "..."`, C++ `TEST(...)`, Go `func Test...`)
        if matches_direct_test_definition(input, i, syntax) {
            attr_buffer.push(("", true));
        }

        // Non-attribute token or start of item
        if !attr_buffer.is_empty() {
            let item_is_test = attr_buffer.iter().any(|(_, is_test)| *is_test);

            if item_is_test {
                attr_buffer.clear();
                skip_test_item_body(input, &mut i);
                out.push(' ');
                continue;
            }

            for (attr, _) in std::mem::take(&mut attr_buffer) {
                out.push_str(attr);
                out.push(' ');
            }
        }

        out.push(ch);
        i = i.saturating_add(ch.len_utf8());
    }

    for (attr, is_test) in attr_buffer {
        if !is_test {
            out.push_str(attr);
            out.push(' ');
        }
    }

    out
}

/// Helper verifying if a relative path is scoped within a directory name at root or declared container package level.
#[must_use]
pub fn matches_directory_scope_with_containers(
    rel_path: &str,
    scope: &str,
    containers: &[String],
) -> bool {
    let clean = scope.trim_matches(['/', '\\']).replace('\\', "/");
    if clean.is_empty() {
        return false;
    }
    if clean == "*" {
        return true;
    }

    let normalized = rel_path.trim_matches(['/', '\\']).replace('\\', "/");

    // 1. Root-level match: e.g. "src/lib.rs" matches "src"
    if normalized == clean || normalized.starts_with(&format!("{clean}/")) {
        return true;
    }

    // 2. Monorepo container match: strictly against declared containers
    let parts: Vec<&str> = normalized.split('/').collect();
    if let (Some(&container), Some(&sub_dir)) = (parts.first(), parts.get(2))
        && sub_dir == clean
        && containers.iter().any(|c| c == container)
    {
        return true;
    }

    false
}

/// Helper verifying if a relative path is scoped within a root-level directory.
#[must_use]
pub fn matches_directory_scope(rel_path: &str, scope: &str) -> bool {
    matches_directory_scope_with_containers(rel_path, scope, &[])
}

/// Helper checking if a path matches a wildcard pattern or glob expression.
#[must_use]
pub fn path_matches_pattern(path_str: &str, pattern: &str) -> bool {
    let p = pattern.trim().replace('\\', "/");
    let target = path_str.trim().replace('\\', "/");

    if p == "*" || p.is_empty() || target == p {
        return true;
    }

    // Directory wildcard e.g. "src/auth/**" or "src/auth/*" matching directory itself or children
    if let Some(prefix) = p.strip_suffix("/**").or_else(|| p.strip_suffix("/*")) {
        let prefix = prefix.trim_end_matches('/');
        if target == prefix || target.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }

    // Direct filename / component match for exact names without wildcards
    if !p.contains('/')
        && let Some(filename) = Path::new(&target).file_name().and_then(|f| f.to_str())
        && filename == p
    {
        return true;
    }

    // Full glob matching via globset (e.g. "src/**/*.rs", "*.{pem,key}", "tests/*_test.rs")
    if p.contains(['*', '?', '[', '{'])
        && let Ok(glob) = globset::GlobBuilder::new(&p)
            .literal_separator(true)
            .build()
    {
        let matcher = glob.compile_matcher();
        if !p.contains('/')
            && let Some(filename) = Path::new(&target).file_name().and_then(|f| f.to_str())
            && matcher.is_match(filename)
        {
            return true;
        }
        if matcher.is_match(&target) {
            return true;
        }
    }

    // Prefix wildcard fallback (e.g. ".env*")
    if let Some(prefix) = p.strip_suffix('*')
        && target.starts_with(prefix)
    {
        return true;
    }

    false
}

/// Returns an error if the agent is restricted and the target path violates the agent's guard boundaries.
///
/// # Errors
/// Returns a descriptive error message if the path violates `disallowed_files`, `disallowed_dirs`,
/// or falls outside `allowed_dirs` and `allowed_files`.
pub fn check_agent_guard_direct(
    workspace_root: &Path,
    candidate: &Path,
    agent_name: Option<&str>,
    guard: Option<&crate::models::AgentGuard>,
) -> Result<(), String> {
    let Some(agent) = agent_name else {
        return Ok(());
    };
    let Some(guard) = guard else {
        return Ok(());
    };

    let relative = candidate.strip_prefix(workspace_root).unwrap_or(candidate);
    let rel_lossy = relative.to_string_lossy();

    let project = crate::project::ProjectConfig::detect_or_load(workspace_root);
    let containers = &project.workspace.containers;

    // 1. Check disallowed files
    for disallowed_file in &guard.disallowed_files {
        if rel_lossy == *disallowed_file {
            return Err(format!(
                "Security policy violation: Agent '{agent}' is not permitted to modify file '{rel_lossy}'."
            ));
        }
    }

    // 2. Check disallowed directories
    for disallowed_dir in &guard.disallowed_dirs {
        if matches_directory_scope_with_containers(&rel_lossy, disallowed_dir, containers) {
            return Err(format!(
                "Security policy violation: Agent '{agent}' is prohibited from modifying files in '{disallowed_dir}/'."
            ));
        }
    }

    // 3. Check allowed files
    let matches_allowed_file = guard.allowed_files.iter().any(|f| rel_lossy == *f);
    if matches_allowed_file {
        return Ok(());
    }

    // 4. Check allowed directories
    if !guard.allowed_dirs.is_empty() {
        if guard.allowed_dirs.iter().any(|d| d == "*") {
            return Ok(());
        }

        let is_allowed = guard
            .allowed_dirs
            .iter()
            .any(|dir| matches_directory_scope_with_containers(&rel_lossy, dir, containers));

        if !is_allowed {
            return Err(format!(
                "Security policy violation: Agent '{agent}' is restricted to directories {:?} or files {:?}, but '{rel_lossy}' was requested.",
                guard.allowed_dirs, guard.allowed_files
            ));
        }
    }

    Ok(())
}

/// Enforces declarative agent path policies (e.g. `comments_only`, `tests_only`, `delimited`).
///
/// Supports language-agnostic syntax resolution via `project.toml` and auto-detection.
///
/// # Errors
/// Returns an error if the agent attempts to modify forbidden content within a policy-guarded path.
pub fn check_agent_path_policies_direct(
    workspace_root: &Path,
    candidate: &Path,
    original: &str,
    new_content: &str,
    agent_name: Option<&str>,
    guard: Option<&crate::models::AgentGuard>,
    project: Option<&crate::project::ProjectConfig>,
) -> Result<(), String> {
    let relative = candidate.strip_prefix(workspace_root).unwrap_or(candidate);
    let rel_lossy = relative.to_string_lossy();

    let effective_proj = project.map_or_else(
        || {
            std::borrow::Cow::Owned(crate::project::ProjectConfig::detect_or_load(
                workspace_root,
            ))
        },
        std::borrow::Cow::Borrowed,
    );
    let containers = &effective_proj.workspace.containers;

    // 1. Collect candidate policies from agent frontmatter
    let mut applicable_policies: Vec<(Option<String>, PathPolicy)> = Vec::new();
    if let Some(g) = guard {
        for p in &g.path_policies {
            if matches_directory_scope_with_containers(&rel_lossy, &p.path, containers)
                || path_matches_pattern(&rel_lossy, &p.path)
            {
                applicable_policies.push((agent_name.map(str::to_owned), p.clone()));
            }
        }
    }

    // 2. Collect candidate policies from project.toml (workspace-wide)
    for p in effective_proj.all_path_policies() {
        if matches_directory_scope_with_containers(&rel_lossy, &p.path, containers)
            || path_matches_pattern(&rel_lossy, &p.path)
        {
            applicable_policies.push((agent_name.map(str::to_owned), p));
        }
    }

    if applicable_policies.is_empty() {
        return Ok(());
    }

    let project_syntax = effective_proj.policies.syntax.as_ref();
    let language = effective_proj.project.language.as_deref();

    for (effective_agent, policy) in &applicable_policies {
        let syntax = policy.effective_syntax(candidate, project_syntax, language);
        policy
            .verify_with_syntax(candidate, original, new_content, &syntax)
            .map_err(|err| {
                let role = effective_agent.as_deref().unwrap_or("operator");
                format!("Security policy violation for agent '{role}': {err}")
            })?;
    }

    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_raw_strings,
    clippy::assertions_on_result_states,
    clippy::indexing_slicing,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_comments_only_allows_docstrings_and_comments() {
        let policy = PathPolicy::comments_only("src");

        let original = "
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
";

        let modified_with_docs = "
/// Adds two numbers together.
///
/// ```
/// assert_eq!(add(1, 2), 3);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    // Perform arithmetic addition
    /* inline comment */
    a + b
}
";

        assert!(policy.verify(original, modified_with_docs).is_ok());
    }

    #[test]
    fn test_python_comments_only_allows_docstrings_and_hash_comments() {
        let policy = PathPolicy {
            path: "app".to_owned(),
            filter: ContentFilterKind::CommentsOnly,
            ..Default::default()
        };
        let syntax = SyntaxConfig {
            line_comments: vec!["#".to_owned()],
            block_comments: vec![
                BlockCommentDelimiters::new("\"\"\"", "\"\"\""),
                BlockCommentDelimiters::new("'''", "'''"),
            ],
            ..Default::default()
        };

        let original = "
def add(a, b):
    return a + b
";

        let with_docs = "
def add(a, b):
    \"\"\"Adds two numbers together.\"\"\"
    # Perform arithmetic addition
    return a + b
";

        let tampered = "
def add(a, b):
    \"\"\"Adds two numbers together.\"\"\"
    return a + b + 1
";

        assert!(
            policy
                .verify_with_syntax(Path::new("app/math.py"), original, with_docs, &syntax)
                .is_ok()
        );
        assert!(
            policy
                .verify_with_syntax(Path::new("app/math.py"), original, tampered, &syntax)
                .is_err()
        );
    }

    #[test]
    fn test_python_tests_only_allows_def_test_and_pytest() {
        let policy = PathPolicy {
            path: "app".to_owned(),
            filter: ContentFilterKind::TestsOnly,
            ..Default::default()
        };
        let syntax = SyntaxConfig {
            line_comments: vec!["#".to_owned()],
            block_comments: vec![BlockCommentDelimiters::new("\"\"\"", "\"\"\"")],
            test_attributes: vec!["@pytest".to_owned()],
            test_prefixes: vec!["test_".to_owned()],
        };

        let original = "
def calculate():
    return 42

def subtract(a, b):
    return a - b
";

        let with_tests = "
def calculate():
    return 42

@pytest.mark.parametrize(\"input\", [1, 2])
def test_calculate(input):
    assert calculate() == 42

def subtract(a, b):
    return a - b

def test_subtract():
    assert subtract(5, 3) == 2
";

        let tampered = "
def calculate():
    return 100

def test_calculate():
    assert calculate() == 100

def subtract(a, b):
    return a - b
";

        assert!(
            policy
                .verify_with_syntax(Path::new("app/math.py"), original, with_tests, &syntax)
                .is_ok()
        );
        assert!(
            policy
                .verify_with_syntax(Path::new("app/math.py"), original, tampered, &syntax)
                .is_err()
        );
    }

    #[test]
    fn test_zig_comments_and_tests() {
        let policy = PathPolicy {
            path: "src".to_owned(),
            filter: ContentFilterKind::TestsOnly,
            ..Default::default()
        };
        let syntax = SyntaxConfig {
            line_comments: vec!["//".to_owned()],
            test_attributes: vec!["test \"".to_owned()],
            ..Default::default()
        };

        let original = "
pub fn calculate() u32 {
    return 42;
}
";

        let with_zig_test = "
pub fn calculate() u32 {
    return 42;
}

test \"calculate returns 42\" {
    try std.testing.expect(calculate() == 42);
}
";

        let tampered = "
pub fn calculate() u32 {
    return 100;
}

test \"calculate returns 42\" {
    try std.testing.expect(calculate() == 100);
}
";

        assert!(
            policy
                .verify_with_syntax(Path::new("src/main.zig"), original, with_zig_test, &syntax)
                .is_ok()
        );
        assert!(
            policy
                .verify_with_syntax(Path::new("src/main.zig"), original, tampered, &syntax)
                .is_err()
        );
    }

    #[test]
    fn test_cpp_tests_only() {
        let policy = PathPolicy {
            path: "src".to_owned(),
            filter: ContentFilterKind::TestsOnly,
            ..Default::default()
        };
        let syntax = SyntaxConfig {
            line_comments: vec!["//".to_owned()],
            block_comments: vec![BlockCommentDelimiters::new("/*", "*/")],
            test_attributes: vec!["TEST(".to_owned()],
            test_prefixes: vec!["test_".to_owned()],
        };

        let original = "
int calculate() {
    return 42;
}
";

        let with_cpp_test = "
int calculate() {
    return 42;
}

TEST(CalcTest, Returns42) {
    EXPECT_EQ(calculate(), 42);
}
";

        let tampered = "
int calculate() {
    return 100;
}

TEST(CalcTest, Returns42) {
    EXPECT_EQ(calculate(), 100);
}
";

        assert!(
            policy
                .verify_with_syntax(Path::new("src/main.cpp"), original, with_cpp_test, &syntax)
                .is_ok()
        );
        assert!(
            policy
                .verify_with_syntax(Path::new("src/main.cpp"), original, tampered, &syntax)
                .is_err()
        );
    }

    #[test]
    fn test_delimited_policy_enforcement() {
        let policy =
            PathPolicy::delimited("src/generated.rs", "// BEGIN AUTOGEN", "// END AUTOGEN");

        let original = "
// Header
// BEGIN AUTOGEN
let version = 1;
// END AUTOGEN
// Footer
";

        let valid_update = r#"
// Header
// BEGIN AUTOGEN
let version = 2;
let build = "alpha";
// END AUTOGEN
// Footer
"#;

        let tampered_footer = "
// Header
// BEGIN AUTOGEN
let version = 2;
// END AUTOGEN
// Tampered Footer
";

        assert!(policy.verify(original, valid_update).is_ok());
        assert!(policy.verify(original, tampered_footer).is_err());

        // Test default markers (TKG:EDITABLE_START / TKG:EDITABLE_END)
        let default_policy = PathPolicy {
            path: "src/tkg.rs".to_owned(),
            filter: ContentFilterKind::Delimited,
            ..Default::default()
        };
        let tkg_orig =
            "// Header\n// TKG:EDITABLE_START\nval = 1\n// TKG:EDITABLE_END\n// Footer\n";
        let tkg_valid =
            "// Header\n// TKG:EDITABLE_START\nval = 2\n// TKG:EDITABLE_END\n// Footer\n";
        let tkg_tampered =
            "// Tampered\n// TKG:EDITABLE_START\nval = 2\n// TKG:EDITABLE_END\n// Footer\n";
        assert!(default_policy.verify(tkg_orig, tkg_valid).is_ok());
        assert!(default_policy.verify(tkg_orig, tkg_tampered).is_err());

        // Test backwards-compatible default markers (AGY:EDITABLE_START / AGY:EDITABLE_END)
        let agy_orig =
            "// Header\n// AGY:EDITABLE_START\nval = 1\n// AGY:EDITABLE_END\n// Footer\n";
        let agy_valid =
            "// Header\n// AGY:EDITABLE_START\nval = 2\n// AGY:EDITABLE_END\n// Footer\n";
        assert!(default_policy.verify(agy_orig, agy_valid).is_ok());
    }
}
