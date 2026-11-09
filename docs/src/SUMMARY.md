# Summary

[Introduction](introduction.md)
[Quickstart](quickstart.md)

# Declarative Fleet Setups & Templates
- [Pre-Built Ecosystem Starter Kits](templates/overview.md)
  - [Go Microservices](templates/go-setup.md)
  - [Python & Data Engineering](templates/python-setup.md)
  - [TypeScript & Fullstack Web](templates/typescript-setup.md)
  - [Standard Rust Systems](templates/rust-setup.md)
  - [Adversarial QA Auditor (tester.md)](templates/qa-tester.md)

# Real-World Workflows & Examples
- [Overview](examples/overview.md)
- [End-to-End Compiler Refactor](examples/compiler-refactor.md)
- [Adding Custom MCP Tools](examples/custom-mcp-tool.md)
- [Isolated Subagent Worktrees](examples/subagent-worktrees.md)

# Architecture & Invariants
- [System Architecture](architecture/overview.md)
- [Configurable Sandboxing & Security Jails](architecture/security-and-sandboxing.md)
- [Multi-Source Session Resumption](architecture/session-resumption.md)

# Engine Subsystems
- [Declarative MCP Daemon (tkg-mcp)](engine/mcp-daemon.md)
- [Lifecycle Interceptors (tkg-hooks)](engine/lifecycle-hooks.md)
- [Flight Control Deck (tkg)](engine/flight-deck.md)

# Fleet & Verification
- [The 10-Agent Fleet](fleet/personas.md)
- [The Verification Sandwich](fleet/verification-sandwich.md)

# Configuration Contracts
- [Agent Manifests (AGENT.md)](configuration/agent-manifest.md)
- [Tools Configuration (tools.toml)](configuration/tools-toml.md)
- [Project Configuration (project.toml)](configuration/project-toml.md)
- [Hooks Configuration (hooks.toml)](configuration/hooks-toml.md)

# Performance & Benchmarks
- [Benchmarks & Latency Profiles](benchmarks.md)
