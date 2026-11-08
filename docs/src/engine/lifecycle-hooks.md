# Lifecycle Interceptors (`agy-hooks`)

`agy-hooks` is an ultra-fast (<1ms startup, <500KB RSS) command-line interceptor executed on Antigravity lifecycle events.

## Intercepted Events

1. **`inject-wake` (`SessionStart` Turn 1 Wake Injection):**  
   Pre-injects prior session handoff summaries, active Jujutsu/Git status, uncommitted diffs, and project documentation paths directly into the initial model prompt without burning an explicit tool call.
2. **`detect-wip` (`PreInvocation` Change Detection):**  
   Emits ephemeral turn-by-turn notification of working tree modifications; burns zero tokens when clean.
3. **`pre-tool-use` (`PreToolUse` Guard Pipeline):**  
   Executes the 6-stage pipeline (`circuit_breaker`, `manifest_guard`, `path_policies`, `subagent_admission`, `vcs_conflicts`, `tool_policy`) to return `allow`, `deny`, or `ask` before any tool runs.
4. **`record-handoff` (`Stop` Session Termination):**  
   Captures session metadata, modified files, and circuit breaker status to `.agy/state/handoff.json` and `.agy/state/last_handoff.md` upon session exit.

## 2-Strike Compiler Circuit Breaker

When an autonomous agent introduces compilation errors, `agy-hooks` tracks consecutive build failures:
* **Strike 1:** Returns structured compiler diagnostics and warns the agent to focus on fixing compiler errors.
* **Strike 2 (Tripped):** Halts automatic tool execution and mandates human intervention or model escalation to prevent loops and token waste.
* **Reset:** The circuit breaker can be reset from `agy-ctrl` with <kbd>c</kbd> or <kbd>Shift+K</kbd>.
