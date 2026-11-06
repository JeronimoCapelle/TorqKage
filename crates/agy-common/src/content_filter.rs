//! Declarative content policy engine for agent-confined file modifications.
//!
//! Enforces mathematical invariant filtering:
//! `strip_allowed_domain(original) == strip_allowed_domain(new_content)`
//!
//! Supports:
//! - `CommentsOnly`: Only docstrings (`///`, `//!`, `/** */`), comments (`//`, `/* */`), and whitespace may change.
//! - `TestsOnly`: Only unit test blocks (`#[cfg(test)] mod ...`, `#[test] fn ...`, `#![cfg(test)]`) and tests may change.
//! - `Delimited`: Only content bounded between explicit `start_marker` and `end_marker` delimiters may change.

use serde::{Deserialize, Serialize};

/// Policy filter kind specifying the allowed modification domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFilterKind {
    /// Allows comments, docstrings (`///`, `//!`, `/* */`), doctests, and formatting whitespace.
    CommentsOnly,
    /// Allows unit test modules (`#[cfg(test)] mod ...`), test items (`#[test] fn ...`), and test blocks.
    TestsOnly,
    /// Allows modifications strictly delimited by configured start and end markers.
    Delimited,
}

/// Declarative path policy binding a workspace path pattern to a content filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

impl PathPolicy {
    /// Constructs a `CommentsOnly` policy for the specified path.
    #[must_use]
    pub fn comments_only(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            filter: ContentFilterKind::CommentsOnly,
            start_marker: None,
            end_marker: None,
        }
    }

    /// Constructs a `TestsOnly` policy for the specified path.
    #[must_use]
    pub fn tests_only(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            filter: ContentFilterKind::TestsOnly,
            start_marker: None,
            end_marker: None,
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
        }
    }

    /// Verifies that changes between `original` and `new_content` comply with this policy.
    ///
    /// # Errors
    /// Returns an error if changes violate the allowed modification domain.
    pub fn verify(&self, original: &str, new_content: &str) -> Result<(), String> {
        match self.filter {
            ContentFilterKind::CommentsOnly => {
                let norm_orig = strip_comments_and_normalize(original);
                let norm_new = strip_comments_and_normalize(new_content);

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
                let norm_orig = strip_tests_and_normalize(
                    original,
                    self.start_marker.as_deref(),
                    self.end_marker.as_deref(),
                );
                let norm_new = strip_tests_and_normalize(
                    new_content,
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

/// Normalizes whitespace runs (spaces, tabs, newlines) into a single ASCII space and trims edges.
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

/// Strips all comments (line `//`, block `/* */`, doc comments `///`, `//!`) from source code,
/// respecting string literals (`"..."`, `r#"..."#`), and normalizes whitespace.
#[must_use]
pub fn strip_comments_and_normalize(input: &str) -> String {
    let stripped = strip_comments(input);
    normalize_whitespace(&stripped)
}

/// Lexer state-machine stripping comments while preserving code tokens and string contents.
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "Char-index state machine lexer operating strictly within bounded vector lengths"
)]
fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0_usize;

    while i < len {
        // 1. Raw string literal check: e.g. r"..." or r#"..."# or br#"..."#
        if let Some((raw_lit, next_idx)) = try_consume_raw_string(&chars, i) {
            out.push_str(&raw_lit);
            i = next_idx;
            continue;
        }

        let ch = chars[i];

        // 2. Regular string literal: "..." or b"..."
        if ch == '"' || (i.saturating_add(1_usize) < len && (ch == 'b' || ch == 'c') && chars[i.saturating_add(1_usize)] == '"') {
            if ch != '"' {
                out.push(ch);
                i = i.saturating_add(1_usize);
            }
            out.push('"');
            i = i.saturating_add(1_usize);

            while i < len {
                let sc = chars[i];
                out.push(sc);
                if sc == '\\' && i.saturating_add(1_usize) < len {
                    i = i.saturating_add(1_usize);
                    out.push(chars[i]);
                } else if sc == '"' {
                    i = i.saturating_add(1_usize);
                    break;
                }
                i = i.saturating_add(1_usize);
            }
            continue;
        }

        // 3. Line comment: // ...
        if ch == '/' && i.saturating_add(1_usize) < len && chars[i.saturating_add(1_usize)] == '/' {
            i = i.saturating_add(2_usize);
            while i < len && chars[i] != '\n' {
                i = i.saturating_add(1_usize);
            }
            out.push(' '); // Separator to prevent accidental token concatenation
            continue;
        }

        // 4. Block comment: /* ... */ with nested block support
        if ch == '/' && i.saturating_add(1_usize) < len && chars[i.saturating_add(1_usize)] == '*' {
            i = i.saturating_add(2_usize);
            let mut depth = 1_usize;
            while i < len && depth > 0_usize {
                if chars[i] == '/' && i.saturating_add(1_usize) < len && chars[i.saturating_add(1_usize)] == '*' {
                    depth = depth.saturating_add(1_usize);
                    i = i.saturating_add(2_usize);
                } else if chars[i] == '*' && i.saturating_add(1_usize) < len && chars[i.saturating_add(1_usize)] == '/' {
                    depth = depth.saturating_sub(1_usize);
                    i = i.saturating_add(2_usize);
                } else {
                    i = i.saturating_add(1_usize);
                }
            }
            out.push(' ');
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

    // Check optional prefix 'b' or 'c'
    if i < len && (chars[i] == 'b' || chars[i] == 'c') {
        i = i.saturating_add(1_usize);
    }

    if i >= len || chars[i] != 'r' {
        return None;
    }
    i = i.saturating_add(1_usize);

    // Count hashes '#'
    let mut hash_count = 0_usize;
    while i < len && chars[i] == '#' {
        hash_count = hash_count.saturating_add(1_usize);
        i = i.saturating_add(1_usize);
    }

    if i >= len || chars[i] != '"' {
        return None;
    }
    i = i.saturating_add(1_usize); // Consume opening quote

    // Now scan until closing quote followed by exact number of hashes
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

/// Strips unit test blocks, items, and attributes, plus optional delimited test sections,
/// and normalizes whitespace.
#[must_use]
pub fn strip_tests_and_normalize(
    input: &str,
    start_marker: Option<&str>,
    end_marker: Option<&str>,
) -> String {
    // 1. If delimited markers configured, strip those first
    let delimited_stripped = if let (Some(start), Some(end)) = (start_marker, end_marker) {
        strip_delimited(input, start, end)
    } else {
        input.to_owned()
    };

    // 2. Strip comments first so attributes and blocks inside comments aren't evaluated
    let no_comments = strip_comments(&delimited_stripped);

    // 3. Check for file-level inner attribute `#![cfg(test)]`
    let trimmed_no_comments = no_comments.trim_start();
    if trimmed_no_comments.starts_with("#![cfg(test")
        || trimmed_no_comments.starts_with("#![cfg(all(test")
    {
        return String::new();
    }

    // 4. Strip Rust test items and attributes
    let tests_stripped = strip_rust_test_items(&no_comments);

    normalize_whitespace(&tests_stripped)
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
            // Unclosed start marker: strip the remainder
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

/// Inspects whether an attribute string (content inside `#[...]`) is a test attribute.
fn is_test_attribute(attr: &str) -> bool {
    let clean = attr.trim();
    if clean == "test" || clean.ends_with("::test") || clean.starts_with("test_case") || clean == "rstest" {
        return true;
    }

    if clean.starts_with("cfg(") && clean.contains("test") {
        return true;
    }

    false
}

/// State-machine lexer that identifies Rust test attributes (`#[test]`, `#[cfg(test)]`, etc.)
/// and consumes the entire associated item (function, module, use statement).
#[allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::iter_with_drain,
    reason = "Item-level brace tracking state machine operating on bounded character vector"
)]
fn strip_rust_test_items(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0_usize;

    // Buffer of consecutive attributes preceding an item: (attr_text, is_test)
    let mut attr_buffer: Vec<(String, bool)> = Vec::new();

    while i < len {
        // Skip leading whitespace between items / attributes, preserving a single space in output if needed
        if chars[i].is_whitespace() {
            if attr_buffer.is_empty() {
                out.push(chars[i]);
            }
            i = i.saturating_add(1_usize);
            continue;
        }

        // Check if starting an outer attribute: #[ ... ]
        if chars[i] == '#' && i.saturating_add(1_usize) < len && chars[i.saturating_add(1_usize)] == '[' {
            let start = i;
            i = i.saturating_add(2_usize); // skip '#['
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
                    // String literal inside attribute (e.g. #[doc = "..."])
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
            let is_test = is_test_attribute(&attr_content);
            attr_buffer.push((full_attr, is_test));
            continue;
        }

        // Non-attribute token encountered: start of an item
        if !attr_buffer.is_empty() {
            let item_is_test = attr_buffer.iter().any(|(_, is_test)| *is_test);

            if item_is_test {
                // Entire item and all its attributes should be stripped!
                attr_buffer.clear();

                // Consume the item: either until balanced `{ ... }` or semicolon `;`
                let mut brace_depth = 0_usize;
                let mut seen_brace = false;

                while i < len {
                    let ch = chars[i];

                    // Respect raw strings inside item
                    if let Some((_, next_idx)) = try_consume_raw_string(&chars, i) {
                        i = next_idx;
                        continue;
                    }

                    // Respect regular strings inside item
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

                    // Respect char literals inside item
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
                                // Consumed the entire brace block
                                break;
                            }
                        } else {
                            i = i.saturating_add(1_usize);
                            break;
                        }
                    } else if ch == ';' && !seen_brace {
                        // Consumed statement item e.g. `use super::*;`
                        i = i.saturating_add(1_usize);
                        break;
                    } else {
                        i = i.saturating_add(1_usize);
                    }
                }

                out.push(' ');
                continue;
            }

            // Not a test item: flush buffered non-test attributes to output
            for (attr, _) in attr_buffer.drain(..) {
                out.push_str(&attr);
                out.push(' ');
            }
        }

        // Emit current character
        out.push(chars[i]);
        i = i.saturating_add(1_usize);
    }

    // Flush any remaining buffered attributes at EOF
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
    fn test_comments_only_allows_docstrings_and_comments() {
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
    fn test_comments_only_rejects_code_modification() {
        let policy = PathPolicy::comments_only("src");

        let original = "
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
";

        let tampered = "
/// Adds two numbers together.
pub fn add(a: i32, b: i32) -> i32 {
    a + b + 1
}
";

        let err = policy.verify(original, tampered).unwrap_err();
        assert!(err.contains("executable code tokens outside comments"));
    }

    #[test]
    fn test_comments_only_preserves_strings_containing_comment_markers() {
        let policy = PathPolicy::comments_only("src");

        let original = r#"
pub fn url() -> &'static str {
    "https://example.com/test//not_a_comment"
}
"#;

        let modified_valid = r#"
/// Returns the target URL.
pub fn url() -> &'static str {
    "https://example.com/test//not_a_comment"
}
"#;

        let tampered_string = r#"
/// Returns the target URL.
pub fn url() -> &'static str {
    "https://evil.com/test//not_a_comment"
}
"#;

        assert!(policy.verify(original, modified_valid).is_ok());
        assert!(policy.verify(original, tampered_string).is_err());
    }

    #[test]
    fn test_tests_only_allows_adding_unit_tests() {
        let policy = PathPolicy::tests_only("src");

        let original = "
pub fn calculate() -> u32 {
    42
}
";

        let with_tests = "
pub fn calculate() -> u32 {
    42
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate() {
        assert_eq!(calculate(), 42);
    }
}
";

        assert!(policy.verify(original, with_tests).is_ok());
    }

    #[test]
    fn test_tests_only_allows_modifying_existing_tests() {
        let policy = PathPolicy::tests_only("src");

        let original = "
pub fn calculate() -> u32 {
    42
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate() {
        assert_eq!(calculate(), 42);
    }
}
";

        let modified_tests = "
pub fn calculate() -> u32 {
    42
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_v2() {
        assert_eq!(calculate(), 42);
        assert!(calculate() > 0);
    }
}
";

        assert!(policy.verify(original, modified_tests).is_ok());
    }

    #[test]
    fn test_tests_only_rejects_production_code_change() {
        let policy = PathPolicy::tests_only("src");

        let original = "
pub fn calculate() -> u32 {
    42
}
";

        let tampered = "
pub fn calculate() -> u32 {
    100
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_calculate() {
        assert_eq!(calculate(), 100);
    }
}
";

        let err = policy.verify(original, tampered).unwrap_err();
        assert!(err.contains("production code outside unit test blocks"));
    }

    #[test]
    fn test_tests_only_allows_whole_test_file_with_inner_cfg_test() {
        let policy = PathPolicy::tests_only("src");

        let original = "";
        let new_test_file = "
#![cfg(test)]

use crate::*;

#[test]
fn test_something() {
    assert!(true);
}
";

        assert!(policy.verify(original, new_test_file).is_ok());
    }

    #[test]
    fn test_tests_only_rejects_new_production_file() {
        let policy = PathPolicy::tests_only("src");

        let original = "";
        let new_prod_file = "
pub fn backdoor() -> bool {
    true
}
";

        assert!(policy.verify(original, new_prod_file).is_err());
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
