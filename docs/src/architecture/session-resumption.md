# Multi-Source Session Resumption

`agy-suite` solves conversational state loss across terminal restarts by providing multi-source session discovery.

## Storage Hierarchy

When `agy-ctrl` selects an agent (e.g. `copilot`), it queries past sessions using embedded `rusqlite`:

1. **Direct Workspace URIs (`conversation_summaries.db`):**  
   Checks if `workspace_uris` matches the canonical workspace path (populated in IDE or multi-root runs).
2. **CLI History Binding (`history.jsonl`):**  
   When `agy` runs interactively from the terminal, `workspace_uris` in SQLite is left empty. The discovery engine correlates the conversation ID against `~/.gemini/antigravity-cli/history.jsonl`, which records the exact working directory (`$PWD`) for each prompt.
3. **FTS5 Workspace Catalog (`conversations_fts`):**  
   Full-text search catalog cross-references the directory name.
4. **Physical Verification:**  
   Ensures that `~/.gemini/antigravity-cli/conversations/<id>.db` physically exists on disk before offering resumption.

## User Interface

In `agy-ctrl`, when a prior session with steps is found:
```text
Resumable (conv: f1bb6355, 50 steps)
[Enter/r: Resume]  [n: New Session]
```
Pressing <kbd>Enter</kbd> or <kbd>r</kbd> invokes:
```bash
agy --conversation <conversation_id>
```
Resuming immediately with full conversation history and context.
