# Introduction

Welcome to **The `Torqkage` Engine Manual**.

> ### Sandboxed Meta-MCP Runtime and Declarative Fleet Engine
> Turn any CLI binary, compiler, or script into an isolated, schema-validated MCP tool with declarative TOML configurations.

`Torqkage` (Antigravity Systems Suite) is a declarative governance engine, Meta-MCP server, featherweight lifecycle runner, and Ratatui 0.30 interactive flight deck designed as an enterprise-grade extension layer for **Google Antigravity** (`agy`).

```
                    ┌──────────────────────────────────────────────┐
                    │               Google Antigravity             │
                    │               (CLI & Core Engine)            │
                    └──────────────────────┬───────────────────────┘
                                           │
                        ┌──────────────────┴──────────────────┐
                        │              Torqkage              │
                        │    (Pure Safe Rust 2024 Workspace)  │
                        └─┬──────────────┬──────────────┬───┬─┘
                          │              │              │   │
        ┌─────────────────┴─┐  ┌─────────┴────────┐  ┌──┴───┴──────────┐  ┌──────────────────┐
        │     tkg      │  │     tkg-mcp      │  │    tkg-hooks    │  │    tkg-common    │
        │ Ratatui TUI Deck  │  │ JSON-RPC Daemon  │  │ Lifecycle Hooks │  │ Shared Primitives│
        └───────────────────┘  └──────────────────┘  └─────────────────┘  └──────────────────┘
```

## Why `Torqkage`?

Autonomous AI coding agents frequently suffer from two fatal failure modes in production systems engineering:
1. **Ambient Shell Poisoning:** Agents run unstructured `bash` commands with full host access, risking accidental `rm -rf`, network leaks, or credential corruption.
2. **Token Exhaustion:** Re-injecting multi-kilobyte interpreted Markdown skills on every turn burns tokens and slows generation.

`Torqkage` replaces ambient, unstructured execution with **deterministic capability perimeters**:
- **Pure Safe Rust:** The entire workspace enforces `#![forbid(unsafe_code)]`.
- **Configurable Container Sandboxing:** Ephemeral Bubblewrap (`bwrap`) isolation toggleable for the entire session in `tkg` or declared per-tool (`enforce_sandbox = true`), featuring private loopback, read-only system roots, and credential masking.
- **Featherweight Daemons:** Sub-millisecond (<1ms, <500KB RSS) lifecycle intercepts and bounded MCP tool execution.
- **Offline Human Cockpit:** Single-pane Ratatui dashboard for toggling permissions, inspecting tools, and resuming previous sessions.
