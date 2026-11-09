#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use tkg_common::{
    AgentFrontmatter, check_dangerous_files_with_security, models::extract_compiler_error_signature,
    project::parse_project_toml, tools::RawToolsConfig,
};

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        drop(AgentFrontmatter::parse(input));
        drop(parse_project_toml(input));
        drop(RawToolsConfig::from_toml_str(input));
        drop(extract_compiler_error_signature(input, None));
        drop(check_dangerous_files_with_security(Path::new(input), None));
    }
});
