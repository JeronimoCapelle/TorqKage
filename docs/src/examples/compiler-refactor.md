# Example: End-to-End Compiler Refactor

This walkthrough illustrates how to navigate a complex systems refactor using the **Verification Sandwich** workflow.

---

## Scenario

You have an open-source parser/compiler project and need to replace an old string-based AST node representation with an enum-based typed node matrix across 15 files.

---

## Step 1: Inception & Contract with `planner`

1. Open `tkg` and select `planner`.
2. Press <kbd>Enter</kbd> to launch.
3. Prompt:
   ```text
   Review our AST definition in src/ast.rs. I want to replace RawToken with a typed
   AstKind enum. Map out the architectural boundary and specify the upfront Verification Strategy.
   ```
4. `planner` audits `src/ast.rs`, checks the public API using `query_public_api`, and writes the architectural transition plan to `ARCHITECTURE.md`.

---

## Step 2: Adversarial Design Verification with `test-author`

1. Return to `tkg`, select `test-author`.
2. Prompt:
   ```text
   Write comprehensive unit and proptest harnesses for the proposed AstKind enum in
   tests/ast_roundtrip.rs before any implementation code is changed.
   ```
3. `test-author` writes property-based tests verifying roundtrip AST serialization. The test suite initially fails to compile or run, establishing our test baseline.

---

## Step 3: Production Systems Implementation with `coder`

1. Return to `tkg`, toggle isolated workspace (<kbd>w</kbd>), and launch `coder`.
2. Prompt:
   ```text
   Implement AstKind across src/ast.rs and src/parser.rs until cargo test passes.
   ```
3. `coder` operates within `.tkg/workspaces/coder` without touching your editor buffers:
   * Rewrites AST structures.
   * Runs `cargo_check` iteratively.
   * If consecutive errors occur, the circuit breaker protects token spend.
   * Once all tests pass, `coder` notifies you.

---

## Step 4: Deterministic Gatekeeping with `finisher`

1. Select `finisher` in `tkg`.
2. Prompt:
   ```text
   Run the pre-release gauntlet. Verify that clippy is clean under -D warnings,
   check cargo-deny, and audit for any mutants.
   ```
3. `finisher` runs `pre_release_gauntlet`, verifies zero warnings, and formats the change description for `git-cliff`.
4. From `copilot`, run `merge_agent_workspace(agent = "coder")` to cleanly merge the tested changes back into `@`.
