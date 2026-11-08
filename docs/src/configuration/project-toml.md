# Project Configuration (`project.toml`)

`project.toml` defines project-specific quality gates and build environments.

## Schema Example

```toml
[project]
name = "my-systems-crate"
ecosystem = "rust"
root_marker = "Cargo.toml"

[quality]
check_command = "cargo check --workspace"
test_command = "cargo test --workspace"
lint_command = "cargo clippy --workspace -- -D warnings"

[documentation]
paths = ["docs", "RFCs"]

[environment]
RUST_BACKTRACE = "1"
```

When present in the root of a repository, `project.toml` configures how `agy-ctrl` and `agy-hooks` validate code changes and run compiler checks.
