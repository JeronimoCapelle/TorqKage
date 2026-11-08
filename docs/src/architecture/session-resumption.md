# Multi-Source Session Resumption

`agy-suite` solves conversational state loss across terminal restarts by providing pluggable, multi-source session discovery.

## Pluggable `SessionProvider` Architecture

Session discovery is abstracted behind the `SessionProvider` trait. Depending on the configured execution harness in `project.toml` (`[runner] harness = "..."`), `agy-ctrl` dynamically selects the appropriate backend:

1. **`AntigravitySessionProvider` (`harness = "agy"`):**  
   - Embedded `rusqlite` query against `~/.gemini/antigravity-cli/conversation_summaries.db`.
   - CLI history correlation against `~/.gemini/antigravity-cli/history.jsonl` matching working directory (`$PWD`).
   - FTS5 workspace catalog resolution.
   - Physical database file verification in `~/.gemini/antigravity-cli/conversations/<id>.db`.

2. **`OpenCodeSessionProvider` (`harness = "opencode"`):**  
   - Discovers workspace sessions in `.opencode/sessions/*.json` and `.opencode/*.json`.
   - Checks global user traces in `~/.local/share/opencode/sessions` and `~/.opencode/sessions`.
   - Extracts session ID, agent filter, message turn counts, and preview descriptions.

3. **`GenericSessionProvider` (`harness = "claude"` or `"custom"`):**  
   - Discovers state records in `.agy/state/sessions/*.json`.

## User Interface

In `agy-ctrl`, when a prior session with steps is found:
```text
Resumable (conv: f1bb6355, 50 steps)
[Enter/r: Resume]  [n: New Session]
```
Pressing <kbd>Enter</kbd> or <kbd>r</kbd> invokes the configured harness resumption flag (e.g. `agy --conversation <id>` or `opencode --session <id>`), resuming immediately with full conversation history and context.
