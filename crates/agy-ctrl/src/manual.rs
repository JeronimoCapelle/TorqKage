//! # manual
//!
//! Integrated offline documentation, architecture manual, and configuration reference for `agy-suite`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Available sections in the integrated manual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManualSection {
    /// Keyboard shortcuts and navigation controls
    Keybindings,
    /// Configuration files specification (`project.toml`, `hooks.toml`, `tools.toml`)
    Configuration,
    /// Filesystem hierarchy and runtime state directories
    Filesystem,
    /// Security model, sandboxing, and defenses
    Security,
    /// Agent fleet roster and specialized roles
    AgentFleet,
}

impl ManualSection {
    pub(crate) const ALL: &'static [Self] = &[
        Self::Keybindings,
        Self::Configuration,
        Self::Filesystem,
        Self::Security,
        Self::AgentFleet,
    ];

    #[must_use]
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Keybindings => "1. Controls & Keybinds",
            Self::Configuration => "2. Configuration Specs",
            Self::Filesystem => "3. Directory Structure",
            Self::Security => "4. Security & Sandboxing",
            Self::AgentFleet => "5. Fleet Architecture",
        }
    }

    #[must_use]
    pub(crate) fn lines(self) -> Vec<Line<'static>> {
        match self {
            Self::Keybindings => keybindings_content(),
            Self::Configuration => configuration_content(),
            Self::Filesystem => filesystem_content(),
            Self::Security => security_content(),
            Self::AgentFleet => agent_fleet_content(),
        }
    }
}

fn h1(text: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        format!(" === {text} === "),
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    ))
}

fn h2(text: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        format!(" --- {text} ---"),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    ))
}

fn key(k: &'static str, desc: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {k:<18}"), Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        Span::styled(desc, Style::default().fg(Color::White)),
    ])
}

fn bullet(tag: &'static str, desc: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  • {tag}: "), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
        Span::styled(desc, Style::default().fg(Color::Gray)),
    ])
}

fn text(t: &'static str) -> Line<'static> {
    Line::from(Span::styled(format!("  {t}"), Style::default().fg(Color::White)))
}

fn empty() -> Line<'static> {
    Line::from("")
}

fn keybindings_content() -> Vec<Line<'static>> {
    vec![
        h1("AGY-CTRL NAVIGATION & SHORTCUT REFERENCE"),
        empty(),
        h2("Global Navigation"),
        key("1 / 2 / 3 / 4", "Switch views: [1] Fleet, [2] Tools, [3] Directives, [4] Manual"),
        key("Tab / BackTab", "Cycle sequentially through views"),
        key("? / F1", "Toggle this Integrated Manual & Documentation"),
        key("q / Ctrl+C", "Quit agy-ctrl control deck"),
        empty(),
        h2("Session Management & Launching"),
        key("Enter", "Resume active agent session (or launch new if none exists)"),
        key("n", "Force launch a fresh, unresumed session for selected agent"),
        key("b / F8", "Toggle outer-loop Bubblewrap sandbox isolation for launched sessions"),
        key("w", "Toggle Jujutsu isolated workspace (.agy/workspaces/<agent>)"),
        key("p", "Cycle agent permission policy: Autonomous -> Supervised -> Planning"),
        key("c / K", "Reset compiler failure circuit breaker (clears consecutive failures)"),
        key("i", "Run in-deck plugin validation, installer, and environment diagnostics"),
        empty(),
        h2("Configuration & Fleet Editing"),
        key("j / k (or ↓ / ↑)", "Navigate agents, tools, or scroll content"),
        key("h / l (or ← / →)", "Toggle focus between Agent List (left) and Tool List (right)"),
        key("Space", "Toggle highlighted tool permission for selected agent"),
        key("g", "Toggle tool permission prompt (ASK operator vs ALLOW autonomous)"),
        key("F2 / S", "Atomic Save & Validate: snapshot VCS, validate plugin, and install"),
        key("F4 / r", "Rollback unsaved modifications to original disk/VCS state"),
        empty(),
        h2("Manual Navigation"),
        key("[ / ]", "Switch manual sections (Controls, Configs, Dirs, Security, Fleet)"),
        key("PageUp / PageDn", "Scroll viewport by 10 lines"),
        empty(),
    ]
}

fn configuration_content() -> Vec<Line<'static>> {
    vec![
        h1("CONFIGURATION FILES & DECLARATIVE SCHEMAS"),
        empty(),
        h2("1. project.toml — Multi-Language Project Contract"),
        text("Located in the workspace root to customize agy-suite without touching engine code:"),
        bullet("[project]", "name, version, and language ecosystem ('rust', 'zig', 'go', 'c')"),
        bullet("[gates.<name>]", "Pre-flight quality gates (command, timeout_seconds, failure_message)"),
        bullet("[gates.linter]", "Invoked before committing changes (e.g. ['cargo', 'clippy'])"),
        bullet("[gates.tests]", "Invoked by test-author/finisher (e.g. ['cargo', 'test'])"),
        bullet("[sandbox]", "Dynamic Bubblewrap mounts (bind_mounts = ['target', '.zig-cache'])"),
        bullet("[environment]", "Per-project environment variables injected into child processes"),
        empty(),
        h2("2. hooks.toml — Lifecycle Hooks Pipeline"),
        text("Defines the PreToolUse safety stages evaluated sequentially on every tool call:"),
        bullet("circuit_breaker", "Trips after consecutive compiler errors, halting mutating tools"),
        bullet("manifest_guard", "Blocks direct edits to Cargo.toml/build.rs to stop proc-macro escape"),
        bullet("subagent_admission", "Checks target agent's frontmatter pre_launch_checks"),
        bullet("vcs_conflicts", "Rejects tool invocations if Jujutsu/Git conflict markers exist"),
        bullet("tool_policy", "Enforces interactive [y/n] prompt on mutating or gated tools"),
        empty(),
        h2("3. tools.toml — Declarative MCP Tool Registry"),
        text("Defines tools exposed by the agy-mcp daemon to subagents:"),
        bullet("action", "static_command, parameterized_command, file_read, file_write, mpatch_edit"),
        bullet("safety", "require_permission_gate, mutating, auto_recover_clean"),
        bullet("sandbox", "enforce_sandbox (bwrap), timeout_seconds, allowed_directories"),
        bullet("preconditions", "require_gates = ['linter', 'tests'] evaluated before tool runs"),
        empty(),
    ]
}

fn filesystem_content() -> Vec<Line<'static>> {
    vec![
        h1("RUNTIME STATE & DIRECTORY ARCHITECTURE"),
        empty(),
        h2("Workspace Hierarchy (.agy/)"),
        bullet(".agy/workspaces/<agent>", "Isolated Jujutsu working copy for each subagent"),
        text("  Prevents file lock collisions and split-brain git index corruption."),
        text("  Clean workspaces are automatically rebased onto default@ on launch."),
        bullet(".agy/state/", "Persistent session state and runtime engine ledgers"),
        text("  • circuit_breaker.json: Failure counter, last error signature, trip status"),
        text("  • active_agent.json: Current running agent scope for MCP permissions"),
        bullet(".agy/artifacts/", "Persistent output artifacts (coverage, benchmarks, flamegraphs)"),
        empty(),
        h2("Cache & Artifact Sharing"),
        bullet(".agy/workspaces/*/target", "Symlinked to root target/ directory"),
        text("  Eliminates redundant cold-start compilation overhead across subagents."),
        text("  Cargo incremental compilation cache is preserved transparently."),
        empty(),
        h2("Plugin Installation Path"),
        bullet("~/.gemini/config/plugins/agy-suite", "Installed plugin directory"),
        text("  Contains agents/, hooks.toml, tools.toml, and plugin manifest."),
        text("  Synchronized atomically via agy-ctrl 'Save & Validate' (F2) or Install (i)."),
        empty(),
    ]
}

fn security_content() -> Vec<Line<'static>> {
    vec![
        h1("SECURITY MODEL & SANDBOX DEFENSES"),
        empty(),
        h2("1. Outer-Loop Bubblewrap Sandboxing"),
        text("Spawns subagent commands inside an isolated Linux container namespace:"),
        bullet("Read-Only Root", "Mounts host /usr, /lib, /bin as read-only (--ro-bind)"),
        bullet("Workspace Confinement", "Mounts only the project root as writable (--bind)"),
        bullet("Network Isolation", "Optionally drops network namespace (--unshare-net)"),
        bullet("Process Group Jail", "Ties lifetime to parent daemon via PR_SET_PDEATHSIG"),
        empty(),
        h2("2. Proc-Macro Host IDE Escape Defense (Manifest Guard)"),
        text("Even inside a sandbox, modifying Cargo.toml or build.rs can compromise the host:"),
        bullet("Attack Vector", "Malicious proc-macro in dependencies is executed by host rust-analyzer"),
        bullet("Neutralization", "hooks.toml [manifest_guard] strictly blocks direct agent mutations"),
        bullet("Protected Files", "Cargo.toml, build.rs, CMakeLists.txt, meson.build, go.mod"),
        empty(),
        h2("3. Compiler Failure Circuit Breaker"),
        text("Prevents runaway token burns when an autonomous loop produces broken code:"),
        bullet("Threshold", "Trips after 2 consecutive compiler/linter failures"),
        bullet("Intervention", "Blocks all mutating tools until human operator approves override"),
        bullet("Manual Reset", "Press 'c' in agy-ctrl or execute 'agy-ctrl reset-breaker'"),
        empty(),
    ]
}

fn agent_fleet_content() -> Vec<Line<'static>> {
    vec![
        h1("AGENT FLEET ROSTER & SPECIALIZATION"),
        empty(),
        bullet("copilot", "Chief of Staff & workflow navigator; coordinates tasks and delegation"),
        bullet("coder", "Autonomous implementation specialist confined strictly to src/"),
        bullet("finisher", "Adversarial gatekeeper auditing diffs, gates, and release readiness"),
        bullet("planner", "Systems design partner maintaining ARCHITECTURE.md and specs"),
        bullet("spec-checker", "Read-only auditor cross-referencing docs against source code and tests"),
        bullet("test-author", "Test suite engineer using nextest, coverage, mutants, insta, fuzz"),
        bullet("perf-analyst", "Performance profiler using cargo-asm, flamegraphs, divan, and Miri"),
        bullet("sysadmin", "Linux administrator troubleshooting Arch packages, bwrap, and services"),
        bullet("mini", "Narrow analytical agent for symbolic math, calculations, and dry runs"),
        empty(),
        h2("Subagent Admission & Pre-Flight Checks"),
        text("Agents declare mandatory pre_launch_checks in YAML frontmatter:"),
        text("  • compilation: Project must compile cleanly before subagent admission"),
        text("  • vcs_conflicts: Zero unresolved Jujutsu/Git merge conflicts permitted"),
        text("  • tests: Test harnesses must compile without error"),
        empty(),
    ]
}
