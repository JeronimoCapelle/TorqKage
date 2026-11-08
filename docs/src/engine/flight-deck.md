# Flight Control Deck (`agy-ctrl`)

`agy-ctrl` is an offline Ratatui 0.30 terminal dashboard providing human operators with full control over the agent fleet, tool permissions, and session lifecycles.

## Primary Tabs

### Tab 1: Fleet Matrix
- **Left Panel:** Displays all 10 registered agents with operational badges (`[AUTO]`, `[ASK]`, `[PLAN]`), tool counts, and modification markers (`*`).
- **Right Panel:** Capability matrix displaying agent description, permission mode, command policy, session resumption telemetry, and a toggleable checklist of MCP tools. Pressing <kbd>Space</kbd> toggles a tool between `ALLOW` (autonomous) and `ASK` (requires operator confirmation).

### Tab 2: MCP Tool Catalog
- Displays all declared tools in `tools.toml`.
- Shows command templates, category badges, execution policies, and timeout bounds.

## Pre-Launch Gatekeeper

When launching an agent from `agy-ctrl`, the pre-launch gatekeeper validates:
1. **Unsaved UI Changes:** Warns if tool permissions were modified in the TUI without saving (<kbd>F2</kbd>).
2. **VCS Conflicts:** Evaluates `jj` or `git` conflict status.
3. **Compiler Checks:** Evaluates `quality.check_command` from `project.toml`.
If any check fails, an interactive modal displays the compiler diagnostics and allows aborting or force-launching.
