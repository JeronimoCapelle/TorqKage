# System Architecture

`TorqKage` is structured as a unified 4-crate Rust workspace:

```
crates/
├── tkg-common/     # Shared systems primitives, Bubblewrap jail builder, ProtoJSON models, root discovery
├── tkg-mcp/        # Persistent JSON-RPC 2.0 stdio MCP daemon providing declarative tool execution
├── tkg-hooks/      # High-performance (<1ms, <500KB RSS) lifecycle hook CLI router
└── tkg/       # Ratatui 0.30 terminal control deck and session orchestrator
```

## The Three Configuration Contracts

The suite replaces hardcoded assumptions with three declarative TOML contracts:

1. **`tools.toml` (MCP Perimeter):**  
   Declares all MCP tools, template inheritance, argument bounds, and default permission gates (`require_permission_gate`).
2. **`project.toml` (Project Perimeter):**  
   Declares language-specific quality gates (`[gates.<name>]`), test harnesses, documentation directories, and custom build environments.
3. **`hooks.toml` (Lifecycle Perimeter):**  
   Defines automated event triggers (e.g. Turn-1 wake context injection, pre-invocation checks, automated circuit breaker tripping).

## Safe Rust Invariant

Every crate in `TorqKage` enforces `#![forbid(unsafe_code)]` at the crate root. Memory safety and concurrency correctness are verified by the Rust compiler without unsafe pointer arithmetic or transmutations.
