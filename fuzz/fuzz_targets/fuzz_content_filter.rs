#![no_main]

use libfuzzer_sys::fuzz_target;
use tkg_common::{
    SyntaxConfig, strip_comments_and_normalize_with_syntax, strip_delimited_and_normalize,
    strip_tests_and_normalize_with_syntax,
};

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let syntax = SyntaxConfig::generic();
        drop(strip_comments_and_normalize_with_syntax(input, &syntax));
        drop(strip_tests_and_normalize_with_syntax(
            input, &syntax, None, None,
        ));
        drop(strip_delimited_and_normalize(input, "/*BEGIN*/", "/*END*/"));
    }
});
