# Standard Rust Systems Starter Kit

The Standard Rust starter kit (`docs/templates/rust/`) is a lightweight, clean baseline for standard Cargo workspaces (without the specialized Cranelift/Mold accelerators of the flagship fleet).

## Directory Structure
```
.agy/
├── project.toml       # `cargo check`, `cargo test`, `clippy -D warnings`, and `fmt`
├── tools.toml         # Sandboxed `cargo_test`, static `cargo_check`, and `cargo_clippy`
├── hooks.toml         # 2-strike compiler circuit breaker and manifest mutation guards
└── agents/
    ├── coder.md       # Implementation agent authoring Rust 2024 code in crates/ and src/
    └── doc-author.md  # Rustdoc and specification author (comments_only in src/)
```

---

## 1. `tools.toml` Highlights
Demonstrates argument targets (`enum = ["--all-targets", "--lib", "--bins", "--examples"]`) and Bubblewrap sandboxing:

```toml
[tools.cargo_test]
description = "Executes cargo test inside a sandboxed Bubblewrap container with target scoping"
action = "parameterized_command"
category = "testing"
enforce_sandbox = true
command = ["cargo", "test"]
args = [
    { name = "target", required = false, default = "--all-targets", enum = ["--all-targets", "--lib", "--bins", "--examples"], description = "Target scope" },
    { name = "package", required = false, flag = "-p", regex = "^[a-zA-Z0-9_-]+$", description = "Specific crate package name" },
    { name = "filter", required = false, flag = "--", regex = "^[a-zA-Z0-9_:]+$", description = "Test name filter" }
]
output_capture = "stdout"
truncate_lines = 100
timeout_seconds = 60
```

---

## 2. In-Place Rustdoc Protection (`comments_only`)
The `doc-author.md` agent is allowed to edit `src/`, but only doc-comments and explanations via the declarative syntax filter:

```yaml
path_policies:
  - path: src
    filter: comments_only
```
Any attempt to mutate executable functions or logic in `src/` is automatically intercepted and rejected by `agy-mcp`.
