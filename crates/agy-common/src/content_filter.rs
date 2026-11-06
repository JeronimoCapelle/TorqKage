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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFilterKind {
    /// Allows comments, docstrings, doctests, and formatting whitespace.
    CommentsOnly,
    /// Allows unit test modules, test items, test functions, and test blocks.
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

impl SyntaxConfig {
    /// Infers default syntax rules based on file extension or language name.
    #[must_use]
    pub fn infer_for_path_or_lang(path: &Path, language: Option<&str>) -> Self {
        if let Some(lang) = language {
            match lang.to_lowercase().as_str() {
                "python" | "py" => return Self::python(),
                "go" | "golang" => return Self::go(),
                "zig" => return Self::zig(),
                "c" | "cpp" | "c++" | "cxx" | "javascript" | "typescript" | "js" | "ts" => {
                    return Self::c_family();
                }
                "shell" | "bash" | "sh" => return Self::shell(),
                "lua" | "sql" => return Self::lua(),
                "rust" | "rs" => return Self::rust(),
                _ => {}
            }
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext.to_lowercase().as_str() {
            "py" | "pyi" => Self::python(),
            "go" => Self::go(),
            "zig" => Self::zig(),
            "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "java" | "cs" | "swift" | "kt"
            | "js" | "ts" | "jsx" | "tsx" => Self::c_family(),
            "sh" | "bash" | "zsh" => Self::shell(),
            "lua" | "sql" => Self::lua(),
            _ => Self::rust(),
        }
    }

    /// Default syntax for Rust.
    #[must_use]
    pub fn rust() -> Self {
        Self {
            line_comments: vec!["//".to_owned()],
            block_comments: vec![BlockCommentDelimiters::new("/*", "*/")],
            test_attributes: vec![
                "test".to_owned(),
                "cfg(test".to_owned(),
                "tokio::test".to_owned(),
                "test_case".to_owned(),
                "rstest".to_owned(),
            ],
            test_prefixes: Vec::new(),
        }
    }

    /// Default syntax for Python.
    #[must_use]
    pub fn python() -> Self {
        Self {
            line_comments: vec!["#".to_owned()],
            block_comments: vec![
                BlockCommentDelimiters::new("\"\"\"", "\"\"\""),
                BlockCommentDelimiters::new("'''", "'''"),
            ],
            test_attributes: vec!["@pytest".to_owned(), "@unittest".to_owned()],
            test_prefixes: vec!["test_".to_owned(), "Test".to_owned()],
        }
    }

    /// Default syntax for C/C++/Java/TypeScript.
    #[must_use]
    pub fn c_family() -> Self {
        Self {
            line_comments: vec!["//".to_owned()],
            block_comments: vec![BlockCommentDelimiters::new("/*", "*/")],
            test_attributes: vec![
                "TEST(".to_owned(),
                "TEST_F(".to_owned(),
                "TEST_P(".to_owned(),
                "@Test".to_owned(),
            ],
            test_prefixes: vec!["test_".to_owned(), "Test".to_owned()],
        }
    }

    /// Default syntax for Go.
    #[must_use]
    pub fn go() -> Self {
        Self {
            line_comments: vec!["//".to_owned()],
            block_comments: vec![BlockCommentDelimiters::new("/*", "*/")],
            test_attributes: vec!["//go:build test".to_owned()],
            test_prefixes: vec![
                "Test".to_owned(),
                "Benchmark".to_owned(),
                "Fuzz".to_owned(),
            ],
        }
    }

    /// Default syntax for Zig.
    #[must_use]
    pub fn zig() -> Self {
        Self {
            line_comments: vec!["//".to_owned()],
            block_comments: Vec::new(),
            test_attributes: vec!["test \"".to_owned()],
            test_prefixes: Vec::new(),
        }
    }

    /// Default syntax for Shell scripts.
    #[must_use]
    pub fn shell() -> Self {
        Self {
            line_comments: vec!["#".to_owned()],
            block_comments: Vec::new(),
            test_attributes: Vec::new(),
            test_prefixes: vec!["test_".to_owned()],
        }
    }

    /// Default syntax for Lua.
    #[must_use]
    pub fn lua() -> Self {
        Self {
            line_comments: vec!["--".to_owned()],
            block_comments: vec![BlockCommentDelimiters::new("--[[", "]]")],
            test_attributes: Vec::new(),
            test_prefixes: vec!["test_".to_owned()],
        }
    }
}

/// Declarative path policy binding a workspace path pattern to a content filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathPolicy {
    /// Relative path or directory pattern (e.g. `"src"`, `"src/lib.rs"`).
    pub path: String,
    /// The content filter kind enforced on matching files.
    pub filter: ContentFilterKind,
    /// Optional agent role this policy binds to (e.g. `"doc-author"`). None = applies to all agents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
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

impl Default for PathPolicy {
    fn default() -> Self {
        Self {
            path: String::new(),
            filter: ContentFilterKind::CommentsOnly,
            agent: None,
            start_marker: None,
            end_marker: None,
            line_comments: Vec::new(),
            block_comments: Vec::new(),
            test_attributes: Vec::new(),
            test_prefixes: Vec::new(),
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
            agent: None,
            start_marker: None,
            end_marker: None,
            line_comments: Vec::new(),
            block_comments: Vec::new(),
            test_attributes: Vec::new(),
            test_prefixes: Vec::new(),
        }
    }

    /// Constructs a `TestsOnly` policy for the specified path.
    #[must_use]
    pub fn tests_only(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            filter: ContentFilterKind::TestsOnly,
            agent: None,
            start_marker: None,
            end_marker: None,
            line_comments: Vec::new(),
            block_comments: Vec::new(),
            test_attributes: Vec::new(),
            test_prefixes: Vec::new(),
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
            agent: None,
            start_marker: Some(start_marker.into()),
            end_marker: Some(end_marker.into()),
            line_comments: Vec::new(),
            block_comments: Vec::new(),
            test_attributes: Vec::new(),
            test_prefixes: Vec::new(),
        }
    }

    /// Resolves the effective `SyntaxConfig` for this policy and candidate path.
    #[must_use]
    pub fn effective_syntax(
        &self,
        candidate_path: &Path,
        project_syntax: Option<&SyntaxConfig>,
        language: Option<&str>,
    ) -> SyntaxConfig {
        let mut base = project_syntax.map_or_else(
            || SyntaxConfig::infer_for_path_or_lang(candidate_path, language),
            Clone::clone,
        );

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

    /// Verifies changes between `original` and `new_content` using default syntax inference.
    ///
    /// # Errors
    /// Returns an error if changes violate the allowed modification domain.
    pub fn verify(&self, original: &str, new_content: &str) -> Result<(), String> {
        let syntax = SyntaxConfig::infer_for_path_or_lang(Path::new(&self.path), None);
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
            }
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
            }
            ContentFilterKind::Delimited => {
                let start = self.start_marker.as_deref().unwrap_or("// AGY:EDITABLE_START");
                let end = self.end_marker.as_deref().unwrap_or("// AGY:EDITABLE_END");

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
            }
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
#[must_use]
pub fn strip_comments_and_normalize(input: &str) -> String {
    let syntax = SyntaxConfig::rust();
    strip_comments_and_normalize_with_syntax(input, &syntax)
}

/// Strips all comments using explicit language syntax and normalizes whitespace.
#[must_use]
pub fn strip_comments_and_normalize_with_syntax(input: &str, syntax: &SyntaxConfig) -> String {
    let stripped = strip_comments_with_syntax(input, syntax);
    normalize_whitespace(&stripped)
}

/// Helper checking if `chars[start..]` starts with `seq`.
#[inline]
fn matches_char_sequence(chars: &[char], start: usize, seq: &[char]) -> bool {
    if start.saturating_add(seq.len()) > chars.len() {
        return false;
    }
    for (offset, &sc) in seq.iter().enumerate() {
        if chars.get(start.saturating_add(offset)) != Some(&sc) {
            return false;
        }
    }
    true
}

/// Lexer state-machine stripping comments while preserving code tokens and string contents.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "Char-index state machine lexer operating strictly within bounded vector lengths"
)]
#[must_use]
pub fn strip_comments_with_syntax(input: &str, syntax: &SyntaxConfig) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0_usize;

    let line_comment_chars: Vec<Vec<char>> = syntax
        .line_comments
        .iter()
        .map(|s| s.chars().collect())
        .filter(|v: &Vec<char>| !v.is_empty())
        .collect();

    let block_comment_chars: Vec<(Vec<char>, Vec<char>)> = syntax
        .block_comments
        .iter()
        .map(|b| (b.start.chars().collect(), b.end.chars().collect()))
        .filter(|(s, e): &(Vec<char>, Vec<char>)| !s.is_empty() && !e.is_empty())
        .collect();

    while i < len {
        // 1. Raw string literal check: e.g. r"..." or r#"..."#
        if let Some((raw_lit, next_idx)) = try_consume_raw_string(&chars, i) {
            out.push_str(&raw_lit);
            i = next_idx;
            continue;
        }

        // 2. Block comments checked before regular strings (handles Python """ before ")
        let mut matched_block = false;
        for (start_seq, end_seq) in &block_comment_chars {
            if matches_char_sequence(&chars, i, start_seq) {
                matched_block = true;
                i = i.saturating_add(start_seq.len());

                if start_seq == end_seq {
                    while i < len {
                        if matches_char_sequence(&chars, i, end_seq) {
                            i = i.saturating_add(end_seq.len());
                            break;
                        }
                        i = i.saturating_add(1_usize);
                    }
                } else {
                    let mut depth = 1_usize;
                    while i < len && depth > 0_usize {
                        if matches_char_sequence(&chars, i, start_seq) {
                            depth = depth.saturating_add(1_usize);
                            i = i.saturating_add(start_seq.len());
                        } else if matches_char_sequence(&chars, i, end_seq) {
                            depth = depth.saturating_sub(1_usize);
                            i = i.saturating_add(end_seq.len());
                        } else {
                            i = i.saturating_add(1_usize);
                        }
                    }
                }

                out.push(' ');
                break;
            }
        }
        if matched_block {
            continue;
        }

        // 3. Line comments
        let mut matched_line = false;
        for lc_seq in &line_comment_chars {
            if matches_char_sequence(&chars, i, lc_seq) {
                matched_line = true;
                i = i.saturating_add(lc_seq.len());
                while i < len && chars[i] != '\n' {
                    i = i.saturating_add(1_usize);
                }
                out.push(' ');
                break;
            }
        }
        if matched_line {
            continue;
        }

        let ch = chars[i];

        // 4. Regular string literal: "..." or '...'
        if ch == '"' || ch == '\'' {
            let quote = ch;
            out.push(quote);
            i = i.saturating_add(1_usize);

            while i < len {
                let sc = chars[i];
                out.push(sc);
                if sc == '\\' && i.saturating_add(1_usize) < len {
                    i = i.saturating_add(1_usize);
                    out.push(chars[i]);
                } else if sc == quote {
                    i = i.saturating_add(1_usize);
                    break;
                }
                i = i.saturating_add(1_usize);
            }
            continue;
        }

        // 5. Normal character
        out.push(ch);
        i = i.saturating_add(1_usize);
    }

    out
}

/// Helper attempting to parse a raw string literal `r#"..."#` starting at `chars[start]`.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "Raw string parsing within bounded character slice"
)]
fn try_consume_raw_string(chars: &[char], start: usize) -> Option<(String, usize)> {
    let len = chars.len();
    let mut i = start;

    if i < len && (chars[i] == 'b' || chars[i] == 'c') {
        i = i.saturating_add(1_usize);
    }

    if i >= len || chars[i] != 'r' {
        return None;
    }
    i = i.saturating_add(1_usize);

    let mut hash_count = 0_usize;
    while i < len && chars[i] == '#' {
        hash_count = hash_count.saturating_add(1_usize);
        i = i.saturating_add(1_usize);
    }

    if i >= len || chars[i] != '"' {
        return None;
    }
    i = i.saturating_add(1_usize);

    while i < len {
        if chars[i] == '"' {
            let mut matched_hashes = 0_usize;
            let mut check_idx = i.saturating_add(1_usize);
            while check_idx < len && matched_hashes < hash_count && chars[check_idx] == '#' {
                matched_hashes = matched_hashes.saturating_add(1_usize);
                check_idx = check_idx.saturating_add(1_usize);
            }
            if matched_hashes == hash_count {
                let end = check_idx;
                let s: String = chars[start..end].iter().collect();
                return Some((s, end));
            }
        }
        i = i.saturating_add(1_usize);
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
    let syntax = SyntaxConfig::rust();
    strip_tests_and_normalize_with_syntax(input, &syntax, start_marker, end_marker)
}

/// Strips unit test blocks using explicit language syntax and normalizes whitespace.
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "Language-agnostic test stripper covering both brace-based and indentation-based definitions"
)]
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
        && (syntax.test_prefixes.iter().any(|p| p.starts_with("test_") || p.starts_with("Test"))
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
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "Indentation line parser operating on bounded lines"
)]
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

        let is_test_attr = syntax.test_attributes.iter().any(|a| trimmed.starts_with(a));
        let is_test_fn = trimmed.starts_with("def ")
            && syntax.test_prefixes.iter().any(|p| {
                trimmed.strip_prefix("def ").unwrap_or("").trim_start().starts_with(p)
            });
        let is_test_class = trimmed.starts_with("class ")
            && syntax.test_prefixes.iter().any(|p| {
                trimmed.strip_prefix("class ").unwrap_or("").trim_start().starts_with(p)
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
#[must_use]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::string_slice,
    reason = "String slice slicing bounded by find indices"
)]
pub fn strip_delimited(input: &str, start_marker: &str, end_marker: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(start_idx) = rest.find(start_marker) {
        out.push_str(&rest[..start_idx]);
        let after_start_offset = start_idx.saturating_add(start_marker.len());
        let after_start = &rest[after_start_offset..];
        if let Some(end_idx) = after_start.find(end_marker) {
            let next_offset = end_idx.saturating_add(end_marker.len());
            rest = &after_start[next_offset..];
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
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::iter_with_drain,
    reason = "Item-level brace tracking state machine operating on bounded character vector"
)]
fn strip_brace_test_items(input: &str, syntax: &SyntaxConfig) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0_usize;

    let mut attr_buffer: Vec<(String, bool)> = Vec::new();

    while i < len {
        if chars[i].is_whitespace() {
            if attr_buffer.is_empty() {
                out.push(chars[i]);
            }
            i = i.saturating_add(1_usize);
            continue;
        }

        // 1. Rust attribute detection: #[ ... ]
        if chars[i] == '#' && i.saturating_add(1_usize) < len && chars[i.saturating_add(1_usize)] == '[' {
            let start = i;
            i = i.saturating_add(2_usize);
            let mut bracket_depth = 1_usize;
            let mut attr_content = String::new();

            while i < len && bracket_depth > 0_usize {
                let ch = chars[i];
                if ch == '[' {
                    bracket_depth = bracket_depth.saturating_add(1_usize);
                    attr_content.push(ch);
                    i = i.saturating_add(1_usize);
                } else if ch == ']' {
                    bracket_depth = bracket_depth.saturating_sub(1_usize);
                    if bracket_depth > 0_usize {
                        attr_content.push(ch);
                    }
                    i = i.saturating_add(1_usize);
                } else if ch == '"' {
                    attr_content.push(ch);
                    i = i.saturating_add(1_usize);
                    while i < len {
                        let sc = chars[i];
                        attr_content.push(sc);
                        if sc == '\\' && i.saturating_add(1_usize) < len {
                            i = i.saturating_add(1_usize);
                            attr_content.push(chars[i]);
                        } else if sc == '"' {
                            i = i.saturating_add(1_usize);
                            break;
                        }
                        i = i.saturating_add(1_usize);
                    }
                } else {
                    attr_content.push(ch);
                    i = i.saturating_add(1_usize);
                }
            }

            let full_attr: String = chars[start..i].iter().collect();
            let is_test = is_matching_test_item(&attr_content, syntax);
            attr_buffer.push((full_attr, is_test));
            continue;
        }

        // 2. Direct test definitions (e.g. Zig `test "..."`, C++ `TEST(...)`, Go `func Test...`)
        let current_slice: String = chars[i..len.min(i.saturating_add(30_usize))].iter().collect();
        let matches_direct_test = syntax.test_attributes.iter().any(|a| current_slice.starts_with(a))
            || syntax.test_prefixes.iter().any(|p| {
                current_slice.starts_with(&format!("func {p}")) || current_slice.starts_with(p)
            });

        if matches_direct_test {
            attr_buffer.push((String::new(), true));
        }

        // Non-attribute token or start of item
        if !attr_buffer.is_empty() {
            let item_is_test = attr_buffer.iter().any(|(_, is_test)| *is_test);

            if item_is_test {
                attr_buffer.clear();

                let mut brace_depth = 0_usize;
                let mut seen_brace = false;

                while i < len {
                    let ch = chars[i];

                    if let Some((_, next_idx)) = try_consume_raw_string(&chars, i) {
                        i = next_idx;
                        continue;
                    }

                    if ch == '"' {
                        i = i.saturating_add(1_usize);
                        while i < len {
                            let sc = chars[i];
                            if sc == '\\' && i.saturating_add(1_usize) < len {
                                i = i.saturating_add(2_usize);
                            } else if sc == '"' {
                                i = i.saturating_add(1_usize);
                                break;
                            } else {
                                i = i.saturating_add(1_usize);
                            }
                        }
                        continue;
                    }

                    if ch == '\'' && i.saturating_add(2_usize) < len && chars[i.saturating_add(2_usize)] == '\'' {
                        i = i.saturating_add(3_usize);
                        continue;
                    }

                    if ch == '{' {
                        seen_brace = true;
                        brace_depth = brace_depth.saturating_add(1_usize);
                        i = i.saturating_add(1_usize);
                    } else if ch == '}' {
                        if brace_depth > 0_usize {
                            brace_depth = brace_depth.saturating_sub(1_usize);
                            i = i.saturating_add(1_usize);
                            if brace_depth == 0_usize {
                                break;
                            }
                        } else {
                            i = i.saturating_add(1_usize);
                            break;
                        }
                    } else if ch == ';' && !seen_brace {
                        i = i.saturating_add(1_usize);
                        break;
                    } else {
                        i = i.saturating_add(1_usize);
                    }
                }

                out.push(' ');
                continue;
            }

            for (attr, _) in attr_buffer.drain(..) {
                out.push_str(&attr);
                out.push(' ');
            }
        }

        out.push(chars[i]);
        i = i.saturating_add(1_usize);
    }

    for (attr, is_test) in attr_buffer.drain(..) {
        if !is_test {
            out.push_str(&attr);
            out.push(' ');
        }
    }

    out
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
        let syntax = SyntaxConfig::python();

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

        assert!(policy.verify_with_syntax(Path::new("app/math.py"), original, with_docs, &syntax).is_ok());
        assert!(policy.verify_with_syntax(Path::new("app/math.py"), original, tampered, &syntax).is_err());
    }

    #[test]
    fn test_python_tests_only_allows_def_test_and_pytest() {
        let policy = PathPolicy {
            path: "app".to_owned(),
            filter: ContentFilterKind::TestsOnly,
            ..Default::default()
        };
        let syntax = SyntaxConfig::python();

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

        assert!(policy.verify_with_syntax(Path::new("app/math.py"), original, with_tests, &syntax).is_ok());
        assert!(policy.verify_with_syntax(Path::new("app/math.py"), original, tampered, &syntax).is_err());
    }

    #[test]
    fn test_zig_comments_and_tests() {
        let policy = PathPolicy {
            path: "src".to_owned(),
            filter: ContentFilterKind::TestsOnly,
            ..Default::default()
        };
        let syntax = SyntaxConfig::zig();

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

        assert!(policy.verify_with_syntax(Path::new("src/main.zig"), original, with_zig_test, &syntax).is_ok());
        assert!(policy.verify_with_syntax(Path::new("src/main.zig"), original, tampered, &syntax).is_err());
    }

    #[test]
    fn test_cpp_tests_only() {
        let policy = PathPolicy {
            path: "src".to_owned(),
            filter: ContentFilterKind::TestsOnly,
            ..Default::default()
        };
        let syntax = SyntaxConfig::c_family();

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

        assert!(policy.verify_with_syntax(Path::new("src/main.cpp"), original, with_cpp_test, &syntax).is_ok());
        assert!(policy.verify_with_syntax(Path::new("src/main.cpp"), original, tampered, &syntax).is_err());
    }

    #[test]
    fn test_delimited_policy_enforcement() {
        let policy = PathPolicy::delimited("src/generated.rs", "// BEGIN AUTOGEN", "// END AUTOGEN");

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
    }
}
