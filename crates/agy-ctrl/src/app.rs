//! Application state machine, navigation, tool toggling, and atomic operations.

use std::path::PathBuf;
use crate::model::{
    AgentManifest, ToolCategory, ToolItem, discover_agents, load_tools_from_file,
    update_tool_permission_gate_in_file,
};
use crate::vcs::{rollback_agent_file, run_plugin_install, run_plugin_validation, save_agent_to_disk, take_vcs_snapshot};

/// Active view tab in the control deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveTab {
    /// Fleet overview and agent capability matrix
    FleetOverview,
    /// Detailed MCP tool catalog and guard inspector
    ToolCatalog,
    /// System prompt and directives editor
    DirectivesEditor,
    /// Integrated architecture manual and documentation
    Manual,
}

impl ActiveTab {
    #[must_use]
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::FleetOverview => "[1] Fleet Matrix",
            Self::ToolCatalog => "[2] Tool Catalog",
            Self::DirectivesEditor => "[3] Directives Editor",
            Self::Manual => "[4] Architecture Manual",
        }
    }
}

/// Focused sub-pane in Tab 1 (Fleet Overview).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FleetFocus {
    /// Agent list (left pane)
    AgentList,
    /// Tool checklist (right pane)
    ToolList,
}

/// Execution configuration flags governing sandbox and workspace isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LaunchConfig {
    pub(crate) sandbox_enabled: bool,
    pub(crate) isolated_workspace: bool,
}

/// The core application state for `agy-ctrl`.
#[derive(Debug)]
pub(crate) struct App {
    pub(crate) workspace_root: PathBuf,
    pub(crate) agents: Vec<AgentManifest>,
    pub(crate) tools: Vec<ToolItem>,
    pub(crate) active_tab: ActiveTab,
    pub(crate) fleet_focus: FleetFocus,
    pub(crate) selected_agent_idx: usize,
    pub(crate) selected_tool_idx: usize,
    pub(crate) selected_category: Option<ToolCategory>,
    pub(crate) directives_scroll: u16,
    pub(crate) manual_section_idx: usize,
    pub(crate) manual_scroll: u16,
    pub(crate) previous_tab: Option<ActiveTab>,
    pub(crate) show_help: bool,
    pub(crate) validation_modal: Option<(bool, String)>,
    pub(crate) status_message: Option<(String, bool)>,
    pub(crate) pending_launch: Option<(String, Option<String>)>,
    pub(crate) active_agent_session: Option<crate::session::SessionSummary>,
    pub(crate) launch_config: LaunchConfig,
    pub(crate) circuit_breaker_status: Option<agy_common::CircuitBreakerLedger>,
    pub(crate) should_quit: bool,
}

impl App {
    #[must_use]
    pub(crate) const fn sandbox_enabled(&self) -> bool {
        self.launch_config.sandbox_enabled
    }

    #[must_use]
    pub(crate) const fn isolated_workspace(&self) -> bool {
        self.launch_config.isolated_workspace
    }

    /// Constructs a new `App` state by discovering agents and tools in `workspace_root`.
    pub(crate) fn new(workspace_root: PathBuf) -> Result<Self, String> {
        let tools_path = workspace_root.join("tools.toml");
        let tools = if tools_path.is_file() {
            load_tools_from_file(&tools_path)?
        } else {
            let plugin_tools = workspace_root.join("plugins/agy-suite/tools.toml");
            if plugin_tools.is_file() {
                eprintln!("[agy-ctrl] Notice: Loaded tools configuration from fallback: {}", plugin_tools.display());
                load_tools_from_file(&plugin_tools)?
            } else if let Ok(home) = std::env::var("HOME") {
                let home_tools = PathBuf::from(home).join(".gemini/config/plugins/agy-suite/tools.toml");
                if home_tools.is_file() {
                    eprintln!("[agy-ctrl] Notice: Loaded tools configuration from installed plugin: {}", home_tools.display());
                    load_tools_from_file(&home_tools)?
                } else {
                    eprintln!("[agy-ctrl] Advisory: No tools.toml found across workspace, plugins, or ~/.gemini/config.");
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        };

        let agents = discover_agents(&workspace_root);

        let circuit_breaker_status = Some(agy_common::CircuitBreakerLedger::load(&workspace_root));

        let mut app = Self {
            workspace_root,
            agents,
            tools,
            active_tab: ActiveTab::FleetOverview,
            fleet_focus: FleetFocus::AgentList,
            selected_agent_idx: 0,
            selected_tool_idx: 0,
            selected_category: None,
            directives_scroll: 0,
            manual_section_idx: 0,
            manual_scroll: 0,
            previous_tab: None,
            show_help: false,
            validation_modal: None,
            status_message: Some(("Press [Enter] Resume, [n] New, [1-4] Views, [?] Manual, [i] Install/Health".to_owned(), false)),
            pending_launch: None,
            active_agent_session: None,
            launch_config: LaunchConfig::default(),
            circuit_breaker_status,
            should_quit: false,
        };
        app.refresh_selected_agent_session();
        Ok(app)
    }

    /// Refreshes the circuit breaker state from disk.
    pub(crate) fn refresh_circuit_breaker(&mut self) {
        self.circuit_breaker_status = Some(agy_common::CircuitBreakerLedger::load(&self.workspace_root));
    }

    /// Toggles Jujutsu workspace isolation (`.agy/workspaces/<agent>`) for launched sessions.
    pub(crate) fn toggle_isolated_workspace(&mut self) {
        self.launch_config.isolated_workspace = !self.launch_config.isolated_workspace;
        let mode = if self.launch_config.isolated_workspace {
            "ENABLED (.agy/workspaces/<agent>)"
        } else {
            "DISABLED (direct repo root)"
        };
        self.status_message = Some((format!("Workspace Isolation: {mode}"), false));
    }

    /// Manually resets the 2-strike compiler failure circuit breaker.
    pub(crate) fn reset_circuit_breaker(&mut self) {
        let mut ledger = agy_common::CircuitBreakerLedger::load(&self.workspace_root);
        if ledger.reset(&self.workspace_root).is_ok() {
            self.circuit_breaker_status = Some(ledger);
            self.status_message = Some(("Circuit Breaker RESET: 0 consecutive failures".to_owned(), false));
        } else {
            self.status_message = Some(("Failed to reset circuit breaker".to_owned(), true));
        }
    }

    /// Discovers the latest resumable session for the selected agent in this workspace.
    pub(crate) fn refresh_selected_agent_session(&mut self) {
        if let Some(agent) = self.selected_agent() {
            self.active_agent_session =
                crate::session::find_agent_session_for_workspace(&agent.name, &self.workspace_root);
        } else {
            self.active_agent_session = None;
        }
    }

    /// Toggles outer-loop Bubblewrap sandbox execution on or off for launched `agy` sessions.
    pub(crate) fn toggle_sandbox(&mut self) {
        self.launch_config.sandbox_enabled = !self.launch_config.sandbox_enabled;
        let mode = if self.launch_config.sandbox_enabled { "ENABLED (bwrap jail)" } else { "DISABLED (direct host)" };
        self.status_message = Some((format!("Terminal Sandbox {mode}"), false));
    }

    /// Cycles the selected agent's permission policy mode between Autonomous, Supervised, and Planning.
    pub(crate) fn cycle_selected_agent_policy(&mut self) {
        if let Some(agent) = self.selected_agent_mut() {
            let desc = agent.cycle_permission_policy();
            let name = agent.name.clone();
            self.status_message = Some((format!("Agent '{name}' policy set to: {desc}"), false));
        }
    }

    /// Toggles whether the highlighted tool requires an interactive permission prompt (ASK vs ALLOW).
    pub(crate) fn toggle_tool_permission_gate(&mut self) {
        let is_tool_tab = matches!(self.active_tab, ActiveTab::ToolCatalog);
        let is_fleet_tool = matches!(self.active_tab, ActiveTab::FleetOverview) && self.fleet_focus == FleetFocus::ToolList;

        if (is_tool_tab || is_fleet_tool)
            && let Some(tool) = self.current_highlighted_tool_mut()
        {
            tool.safety.require_permission_gate = !tool.safety.require_permission_gate;
            tool.modified = true;
            let state = if tool.require_permission_gate() { "ASK (Interactive confirmation)" } else { "ALLOW (Autonomous execution)" };
            let name = tool.name.clone();
            self.status_message = Some((format!("Tool '{name}' permission gate: {state}"), false));
        }
    }

    /// Requests launching an interactive `agy` session for the currently selected agent.
    /// If `resume` is true and a previous session exists for this workspace, resumes it.
    pub(crate) fn request_launch(&mut self, resume: bool) {
        if let Some(agent) = self.selected_agent() {
            let resume_id = if resume {
                self.active_agent_session
                    .as_ref()
                    .map(|s| s.conversation_id.clone())
            } else {
                None
            };
            self.pending_launch = Some((agent.name.clone(), resume_id));
        }
    }

    /// Returns a reference to the currently selected agent, if any.
    #[must_use]
    pub(crate) fn selected_agent(&self) -> Option<&AgentManifest> {
        self.agents.get(self.selected_agent_idx)
    }

    /// Returns a mutable reference to the currently selected agent, if any.
    pub(crate) fn selected_agent_mut(&mut self) -> Option<&mut AgentManifest> {
        self.agents.get_mut(self.selected_agent_idx)
    }

    /// Moves selection to the next agent.
    pub(crate) fn next_agent(&mut self) {
        if !self.agents.is_empty() {
            let next = self.selected_agent_idx.saturating_add(1);
            if next >= self.agents.len() {
                self.selected_agent_idx = 0;
            } else {
                self.selected_agent_idx = next;
            }
            self.directives_scroll = 0;
            self.refresh_selected_agent_session();
        }
    }

    /// Moves selection to the previous agent.
    pub(crate) fn prev_agent(&mut self) {
        if !self.agents.is_empty() {
            if self.selected_agent_idx == 0 {
                self.selected_agent_idx = self.agents.len().saturating_sub(1);
            } else {
                self.selected_agent_idx = self.selected_agent_idx.saturating_sub(1);
            }
            self.directives_scroll = 0;
            self.refresh_selected_agent_session();
        }
    }

    /// Moves selection to the next tool in the active tool list.
    pub(crate) fn next_tool(&mut self) {
        let total = self.active_tool_count();
        if total > 0 {
            let next = self.selected_tool_idx.saturating_add(1);
            if next >= total {
                self.selected_tool_idx = 0;
            } else {
                self.selected_tool_idx = next;
            }
        }
    }

    /// Moves selection to the previous tool in the active tool list.
    pub(crate) fn prev_tool(&mut self) {
        let total = self.active_tool_count();
        if total > 0 {
            if self.selected_tool_idx == 0 {
                self.selected_tool_idx = total.saturating_sub(1);
            } else {
                self.selected_tool_idx = self.selected_tool_idx.saturating_sub(1);
            }
        }
    }

    /// Counts how many tools are in the active view (filtered or unfiltered).
    #[must_use]
    pub(crate) fn active_tool_count(&self) -> usize {
        match self.active_tab {
            ActiveTab::FleetOverview => self.tools.len(),
            ActiveTab::ToolCatalog => self
                .selected_category
                .map_or(self.tools.len(), |cat| self.tools.iter().filter(|t| t.category == cat).count()),
            ActiveTab::DirectivesEditor | ActiveTab::Manual => 0,
        }
    }

    /// Returns the currently highlighted tool item based on active filtering.
    #[must_use]
    pub(crate) fn current_highlighted_tool(&self) -> Option<&ToolItem> {
        match self.active_tab {
            ActiveTab::FleetOverview => self.tools.get(self.selected_tool_idx),
            ActiveTab::ToolCatalog => self.selected_category.map_or_else(
                || self.tools.get(self.selected_tool_idx),
                |cat| self.tools.iter().filter(|t| t.category == cat).nth(self.selected_tool_idx),
            ),
            ActiveTab::DirectivesEditor | ActiveTab::Manual => None,
        }
    }

    /// Returns a mutable reference to the currently highlighted tool item based on active filtering.
    pub(crate) fn current_highlighted_tool_mut(&mut self) -> Option<&mut ToolItem> {
        let idx = self.selected_tool_idx;
        match self.active_tab {
            ActiveTab::FleetOverview => self.tools.get_mut(idx),
            ActiveTab::ToolCatalog => {
                if let Some(cat) = self.selected_category {
                    self.tools.iter_mut().filter(|t| t.category == cat).nth(idx)
                } else {
                    self.tools.get_mut(idx)
                }
            }
            ActiveTab::DirectivesEditor | ActiveTab::Manual => None,
        }
    }

    /// Toggles the currently highlighted tool on or off for the active agent.
    #[allow(clippy::collapsible_if, reason = "Disjoint borrowing of self requires sequential if-let blocks")]
    pub(crate) fn toggle_current_tool(&mut self) {
        if let Some(tool) = self.current_highlighted_tool().cloned() {
            if let Some(agent) = self.selected_agent_mut() {
                agent.toggle_mcp_tool(&tool.name);
                let allowed = agent.allows_mcp_tool(&tool.name);
                self.status_message = Some((
                    format!(
                        "Tool '{}' {} for agent '{}'",
                        tool.name,
                        if allowed { "GRANTED" } else { "REMOVED" },
                        agent.name
                    ),
                    false,
                ));
            }
        }
    }

    /// Switches focus between left and right sub-panes in Tab 1.
    pub(crate) const fn toggle_fleet_focus(&mut self) {
        self.fleet_focus = match self.fleet_focus {
            FleetFocus::AgentList => FleetFocus::ToolList,
            FleetFocus::ToolList => FleetFocus::AgentList,
        };
    }

    /// Advances to the next tab.
    pub(crate) const fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::FleetOverview => ActiveTab::ToolCatalog,
            ActiveTab::ToolCatalog => ActiveTab::DirectivesEditor,
            ActiveTab::DirectivesEditor => ActiveTab::Manual,
            ActiveTab::Manual => ActiveTab::FleetOverview,
        };
        self.selected_tool_idx = 0;
    }

    /// Returns to the previous tab.
    pub(crate) const fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::FleetOverview => ActiveTab::Manual,
            ActiveTab::ToolCatalog => ActiveTab::FleetOverview,
            ActiveTab::DirectivesEditor => ActiveTab::ToolCatalog,
            ActiveTab::Manual => ActiveTab::DirectivesEditor,
        };
        self.selected_tool_idx = 0;
    }

    /// Toggles between the current tab and the Integrated Manual tab.
    pub(crate) fn toggle_manual(&mut self) {
        if self.active_tab == ActiveTab::Manual {
            if let Some(prev) = self.previous_tab.take() {
                self.active_tab = prev;
            } else {
                self.active_tab = ActiveTab::FleetOverview;
            }
        } else {
            self.previous_tab = Some(self.active_tab);
            self.active_tab = ActiveTab::Manual;
        }
    }

    /// Advances to the next manual section.
    pub(crate) const fn next_manual_section(&mut self) {
        let total = crate::manual::ManualSection::ALL.len();
        let next = self.manual_section_idx.saturating_add(1);
        if next >= total {
            self.manual_section_idx = 0;
        } else {
            self.manual_section_idx = next;
        }
        self.manual_scroll = 0;
    }

    /// Returns to the previous manual section.
    pub(crate) const fn prev_manual_section(&mut self) {
        let total = crate::manual::ManualSection::ALL.len();
        if self.manual_section_idx == 0 {
            self.manual_section_idx = total.saturating_sub(1);
        } else {
            self.manual_section_idx = self.manual_section_idx.saturating_sub(1);
        }
        self.manual_scroll = 0;
    }

    /// Runs in-deck plugin installation and system health diagnostics.
    pub(crate) fn run_installer_and_diagnostics(&mut self) {
        match crate::installer::install_plugin_interactive(&self.workspace_root) {
            Ok(msg) => {
                let diag = crate::installer::run_diagnostics(&self.workspace_root);
                self.validation_modal = Some((true, format!("{msg}\n\n{}", diag.summary())));
                self.status_message = Some(("✔ Plugin installed & diagnostics updated!".to_owned(), false));
            }
            Err(err) => {
                let diag = crate::installer::run_diagnostics(&self.workspace_root);
                self.validation_modal = Some((false, format!("Failed to install plugin: {err}\n\n{}", diag.summary())));
                self.status_message = Some(("❌ Plugin installation failed! Check diagnostics.".to_owned(), true));
            }
        }
    }

    /// Executes the atomic "Save & Validate" pipeline (F2).
    pub(crate) fn save_and_validate(&mut self) {
        let modified_agents = self.agents.iter().filter(|a| a.modified).count();
        let modified_tools = self.tools.iter().filter(|t| t.modified).count();
        if modified_agents == 0 && modified_tools == 0 {
            self.status_message = Some(("No changes to save - all agent and tool configs are clean".to_owned(), false));
            return;
        }

        // 1. Take VCS snapshot
        drop(take_vcs_snapshot(
            &self.workspace_root,
            &format!("snapshot before saving {modified_agents} agent configs and {modified_tools} tool modifications"),
        ));

        // 2. Save modified tool permission gates to disk
        let tools_path = self.workspace_root.join("tools.toml");
        let plugin_tools_path = self.workspace_root.join("plugins/agy-suite/tools.toml");
        for tool in &mut self.tools {
            if !tool.modified {
                continue;
            }
            drop(update_tool_permission_gate_in_file(&tools_path, &tool.name, tool.require_permission_gate()));
            drop(update_tool_permission_gate_in_file(&plugin_tools_path, &tool.name, tool.require_permission_gate()));
            tool.modified = false;
        }

        // 3. Save modified manifests to disk
        for agent in &mut self.agents {
            if !agent.modified {
                continue;
            }
            if let Err(err) = save_agent_to_disk(agent) {
                self.validation_modal = Some((false, format!("Failed to save {}:\n{err}", agent.file_path.display())));
                return;
            }
        }

        // 4. Run validation
        match run_plugin_validation(&self.workspace_root) {
            Ok(report) => {
                // 5. Install plugin on success
                let install_out = run_plugin_install(&self.workspace_root).unwrap_or_default();
                for agent in &mut self.agents {
                    agent.modified = false;
                }
                self.validation_modal = Some((
                    true,
                    format!("✔ Pre-flight validation passed!\n\n{report}\n\nReinstalled successfully:\n{install_out}"),
                ));
                self.status_message = Some(("✔ Saved, validated, and reinstalled successfully!".to_owned(), false));
            }
            Err(err_report) => {
                self.validation_modal = Some((
                    false,
                    format!("🛑 Validation Failed!\n\n{err_report}\n\nPress F4 or 'r' to rollback changes."),
                ));
                self.status_message = Some(("❌ Validation failed! Check diagnostic modal.".to_owned(), true));
            }
        }
    }

    /// Reverts modified agent configurations to disk or VCS state (F4 / r).
    pub(crate) fn rollback(&mut self) {
        let mut restored: usize = 0;
        for agent in &mut self.agents {
            if agent.modified && rollback_agent_file(&self.workspace_root, &agent.file_path).is_ok() {
                restored = restored.saturating_add(1);
            }
        }

        // Reload tools from disk if tools were modified
        let tools_path = self.workspace_root.join("tools.toml");
        let plugin_tools = self.workspace_root.join("plugins/agy-suite/tools.toml");
        let candidate = if tools_path.is_file() {
            tools_path
        } else if plugin_tools.is_file() {
            plugin_tools
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".gemini/config/plugins/agy-suite/tools.toml")
        } else {
            tools_path
        };

        if candidate.is_file()
            && let Ok(loaded) = load_tools_from_file(&candidate)
        {
            self.tools = loaded;
        }

        // Reload agents from disk
        self.agents = discover_agents(&self.workspace_root);

        self.validation_modal = None;
        self.status_message = Some((
            format!("Rollback complete: restored {restored} agent manifests to previous state"),
            false,
        ));
    }
}
