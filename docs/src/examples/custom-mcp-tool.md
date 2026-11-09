# Example: Adding a Custom Declarative Tool

Adding new capabilities to `Torqkage` requires **zero Rust recompilation**. Everything is defined declaratively in `tools.toml`.

---

## Scenario: Adding `cargo-bloat` to Inspect Binary Size

Suppose you want to give `perf-analyst` the ability to inspect binary size and identify which functions consume the most bytes.

---

## Step 1: Add Tool Definition to `tools.toml`

Open `tools.toml` (or `plugins/torqkage/tools.toml`) and declare the tool under `[tools]`:

```toml
[tools.cargo_bloat]
inherit = "base_tools.perf_tool"
action = "static_command"
command = ["cargo", "bloat", "--release", "-n", "20"]
description = "Analyzes binary size and lists the top 20 functions consuming the most space"
require_permission_gate = false
enforce_sandbox = true
```

### Explaining the Fields
- `inherit = "base_tools.perf_tool"`: Inherits base configuration, 60s timeout, and `systems` category.
- `action = "static_command"`: An exact, zero-argument command safely executed.
- `enforce_sandbox = true`: Explicitly runs this tool inside an isolated Bubblewrap container.
- `require_permission_gate = false`: Allows the agent to run it autonomously (`ALLOW`).

---

## Step 2: Grant the Tool in the Agent Manifest

Open `plugins/torqkage/agents/perf-analyst.md` and add `cargo_bloat` to the `mcp_tools` YAML list:

```yaml
---
name: perf-analyst
description: Hardware performance profiler and benchmark engineer
effort: high
permissionMode: plan
mcp_tools:
  - read_project_file
  - cargo_asm
  - cargo_bloat      # <-- Added custom tool
---
```

---

## Step 3: Synchronize & Validate

In `tkg`, press <kbd>i</kbd> to reinstall and validate the plugin bundle in-place:
```bash
tkg  # In-deck reinstall via 'i'
```

Alternatively, from the shell:
```bash
./install.sh --fast
```

The tool is immediately available to `perf-analyst` via the JSON-RPC daemon with complete parameter sanitization and container isolation.

---

## Advanced Scenario: Parameterized Tool with Sandboxing & Enums

To declare dynamic tools that take user parameters safely, define `action = "parameterized_command"` with typed `args`:

```toml
[tools.run_benchmark]
description = "Executes benchmark target inside an isolated Bubblewrap container"
action = "parameterized_command"
category = "profiling"
enforce_sandbox = true
command = ["cargo", "bench"]
args = [
    { name = "target", required = false, flag = "--bench", regex = "^[a-zA-Z0-9_-]+$", description = "Specific benchmark target name" },
    { name = "mode", required = false, default = "quick", enum = ["quick", "full", "profile"], description = "Benchmark sampling mode" },
    { name = "verbose", required = false, flag = "--verbose", type = "boolean", description = "Emit verbose timing distributions" }
]
output_capture = "stdout"
truncate_lines = 150
timeout_seconds = 120
```

The engine automatically:
1. Synthesizes an MCP-compliant JSON Schema with `mode` restricted strictly to the declared `enum` choices.
2. Validates incoming arguments against regex before executing commands.
3. Jails the process inside a Bubblewrap container (`enforce_sandbox = true`) with private loopback and zero external network access.
