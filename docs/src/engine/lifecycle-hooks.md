# Lifecycle Interceptors (`agy-hooks`)

`agy-hooks` is an ultra-fast (<1ms startup, <500KB RSS) command-line interceptor executed on Antigravity lifecycle events.

## Intercepted Events

1. **`detect-wip` (Turn 1 Wake Injection):**  
   Pre-injects working copy status (e.g. `jj status` or `git status`) and uncommitted diffs directly into the initial model prompt without burning an explicit tool call.
2. **`check-plan` (Pre-Tool Use Interceptor):**  
   Validates agent arguments against declarative path policies (`tests_only`, `comments_only`, `restricted_paths`) before the tool executes.
3. **`record-handoff` (Post-Invocation Interceptor):**  
   Records agent activity, updates circuit breaker status, and logs telemetry to `.agy/state/`.

## 2-Strike Compiler Circuit Breaker

When an autonomous agent introduces compilation errors, `agy-hooks` tracks consecutive build failures:
* **Strike 1:** Returns structured compiler diagnostics and warns the agent to focus on fixing compiler errors.
* **Strike 2 (Tripped):** Halts automatic tool execution and mandates human intervention or model escalation to prevent loops and token waste.
* **Reset:** The circuit breaker can be reset from `agy-ctrl` with <kbd>c</kbd> or <kbd>Shift+K</kbd>.
