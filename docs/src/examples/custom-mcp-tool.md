# Example: Adding a Custom Declarative Tool

Adding new capabilities to `agy-suite` requires **zero Rust recompilation**. Everything is defined declaratively in `tools.toml`.

---

## Scenario: Adding `cargo-bloat` to Inspect Binary Size

Suppose you want to give `perf-analyst` the ability to inspect binary size and identify which functions consume the most bytes.

---

## Step 1: Add Tool Definition to `tools.toml`

Open `tools.toml` (or `plugins/agy-suite/tools.toml`) and declare the tool under `[tools]`:

```toml
[tools.cargo_bloat]
template = "perf_tool"
action = "static_command"
command = "cargo bloat --release -n 20"
description = "Analyzes binary size and lists the top 20 functions consuming the most space"
require_permission_gate = false
```

### Explaining the Fields
- `template = "perf_tool"`: Inherits default Bubblewrap sandboxing, 60s timeout, and `systems` category.
- `action = "static_command"`: An exact, zero-argument command safely executed inside the container.
- `require_permission_gate = false`: Allows the agent to run it autonomously (`ALLOW`).

---

## Step 2: Grant the Tool in the Agent Manifest

Open `plugins/agy-suite/agents/perf-analyst.md` and add `cargo_bloat` to the `mcp_tools` YAML list:

```yaml
---
name: perf-analyst
description: Hardware performance profiler and benchmark engineer
effort: high
permission_mode: plan
mcp_tools:
  - read_project_file
  - cargo_asm
  - cargo_bloat      # <-- Added custom tool
---
```

---

## Step 3: Synchronize & Validate

In `agy-ctrl`, press <kbd>i</kbd> to reinstall and validate the plugin bundle in-place:
```bash
agy-ctrl  # In-deck reinstall via 'i'
```

Alternatively, from the shell:
```bash
./install.sh --fast
```

The tool is immediately available to `perf-analyst` via the JSON-RPC daemon with complete parameter sanitization and container isolation.
