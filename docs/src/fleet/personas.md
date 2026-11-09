# The 10-Agent Fleet

`Torqkage` provides 10 specialized agent archetypes configured in `plugins/TorqKage/agents/`:

| Agent | Role | Default Mode | Key MCP Capabilities |
| :--- | :--- | :--- | :--- |
| **`coder`** | Production systems implementation | `acceptEdits` | Code modification, AST rewriting, compiler diagnostics |
| **`copilot`** | Chief of Staff & workflow navigator | `plan` | Workspace status, diff inspection, subagent delegation |
| **`doc-author`** | Technical documentation & rustdoc | `acceptEdits` | Markdown editing, rustdoc generation |
| **`finisher`** | Pre-release gatekeeper & release auditor | `plan` | `cargo-deny`, `cargo-mutants`, release gauntlet |
| **`mini`** | Symbolic & algebraic calculation | `acceptEdits` | Python SymPy symbolic calculation |
| **`perf-analyst`**| Hardware profiler & benchmark engineer | `plan` | `llvm-mca`, `cargo-asm`, `flamegraph`, hyperfine |
| **`planner`** | Systems design & architecture partner | `acceptEdits` | Public API auditing, architecture mapping |
| **`spec-checker`**| Read-only auditor cross-referencing specs | `plan` | `llvm-objdump`, semver checking, spec audits |
| **`sysadmin`** | Arch Linux system administrator | `acceptEdits` | Diagnostic lookups, ground-truth CLI help |
| **`test-author`** | Design Verification (DV) & Formal Test | `acceptEdits` | Kani formal proofs, Loom, Miri, Proptest |

## Authoring Custom Agents

To create a custom agent, add `<name>.md` to `plugins/TorqKage/agents/`:

```yaml
---
name: my-agent
description: Specialized verification agent
effort: high
permission_mode: plan
command_execution_policy: bwrap_isolated
mcp_tools:
  - read_project_file
  - cargo_check
---
You are a specialized verification agent...
```
