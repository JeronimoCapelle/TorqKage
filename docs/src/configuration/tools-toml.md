# Tools Configuration (`tools.toml`)

`tools.toml` defines the declarative MCP tool catalog.

## Schema Example

```toml
[templates.rust_tool]
category = "verification"
execution_policy = "bwrap_isolated"
timeout_secs = 60

[tools.cargo_check]
template = "rust_tool"
action = "static_command"
command = "cargo check --workspace --all-targets"
description = "Fast syntax, type, and borrow check across all workspace crates"
require_permission_gate = false

[tools.modify_code]
action = "mpatch_edit"
description = "High-precision search-and-replace code modification"
require_permission_gate = true
```

## Tool Categories
- `core`: File reading, project exploration, documentation lookups.
- `edit`: Code modification and writing.
- `verification`: Static analysis, unit tests, formal verification.
- `systems`: Hardware profiling, disassembly, benchmarking.
- `orchestration`: Subagent workspace creation, git/jj inspection.
