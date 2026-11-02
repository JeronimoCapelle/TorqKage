//! Application state machine, navigation, tool toggling, and atomic operations.

use std::path::PathBuf;
use crate::model::{AgentManifest, ToolCategory, ToolItem, discover_agents, load_tools_from_file};
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
}

impl ActiveTab {
    #[must_use]
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::FleetOverview => "[1] Fleet Matrix",
            Self::ToolCatalog => "[2] Tool Catalog",
            Self::DirectivesEditor => "[3] Directives Editor",
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
    pub(crate) show_help: bool,
    pub(crate) validation_modal: Option<(bool, String)>,
    pub(crate) status_message: Option<(String, bool)>,
    pub(crate) pending_launch: Option<String>,
    pub(crate) sandbox_enabled: bool,
    pub(crate) should_quit: bool,
}

impl App {
    /// Constructs a new `App` state by discovering agents and tools in `workspace_root`.
    pub(crate) fn new(workspace_root: PathBuf) -> Result<Self, String> {
        let tools_path = workspace_root.join("tools.toml");
        let tools = if tools_path.is_file() {
            load_tools_from_file(&tools_path)?
        } else {
            let plugin_tools = workspace_root.join("plugins/agy-suite/tools.toml");
            if plugin_tools.is_file() {
                load_tools_from_file(&plugin_tools)?
            } else if let Ok(home) = std::env::var("HOME") {
                let home_tools = PathBuf::from(home).join(".gemini/config/plugins/agy-suite/tools.toml");
                if home_tools.is_file() {
                    load_tools_from_file(&home_tools)?
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        };

        let agents = discover_agents(&workspace_root);

        Ok(Self {
            workspace_root,
            agents,
            tools,
            active_tab: ActiveTab::FleetOverview,
            fleet_focus: FleetFocus::AgentList,
            selected_agent_idx: 0,
            selected_tool_idx: 0,
            selected_category: None,
            directives_scroll: 0,
            show_help: false,
            validation_modal: None,
            status_message: Some(("Press [Enter/l] Launch agy, [p] Policy, [s] Sandbox, [F2] Save".to_owned(), false)),
            pending_launch: None,
            sandbox_enabled: false,
            should_quit: false,
        })
    }

    /// Toggles outer-loop Bubblewrap sandbox execution on or off for launched `agy` sessions.
    pub(crate) fn toggle_sandbox(&mut self) {
        self.sandbox_enabled = !self.sandbox_enabled;
        let mode = if self.sandbox_enabled { "ENABLED (bwrap jail)" } else { "DISABLED (direct host)" };
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
            tool.require_permission_gate = !tool.require_permission_gate;
            let state = if tool.require_permission_gate { "ASK (Interactive confirmation)" } else { "ALLOW (Autonomous execution)" };
            let name = tool.name.clone();
            self.status_message = Some((format!("Tool '{name}' permission gate: {state}"), false));
        }
    }

    /// Requests launching an interactive `agy` session for the currently selected agent.
    pub(crate) fn request_launch(&mut self) {
        if let Some(agent) = self.selected_agent() {
            self.pending_launch = Some(agent.name.clone());
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
    pub(crate) const fn next_agent(&mut self) {
        if !self.agents.is_empty() {
            let next = self.selected_agent_idx.saturating_add(1);
            if next >= self.agents.len() {
                self.selected_agent_idx = 0;
            } else {
                self.selected_agent_idx = next;
            }
            self.directives_scroll = 0;
        }
    }

    /// Moves selection to the previous agent.
    pub(crate) const fn prev_agent(&mut self) {
        if !self.agents.is_empty() {
            if self.selected_agent_idx == 0 {
                self.selected_agent_idx = self.agents.len().saturating_sub(1);
            } else {
                self.selected_agent_idx = self.selected_agent_idx.saturating_sub(1);
            }
            self.directives_scroll = 0;
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
            ActiveTab::DirectivesEditor => 0,
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
            ActiveTab::DirectivesEditor => None,
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
            ActiveTab::DirectivesEditor => None,
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
            ActiveTab::DirectivesEditor => ActiveTab::FleetOverview,
        };
        self.selected_tool_idx = 0;
    }

    /// Returns to the previous tab.
    pub(crate) const fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::FleetOverview => ActiveTab::DirectivesEditor,
            ActiveTab::ToolCatalog => ActiveTab::FleetOverview,
            ActiveTab::DirectivesEditor => ActiveTab::ToolCatalog,
        };
        self.selected_tool_idx = 0;
    }

    /// Executes the atomic "Save & Validate" pipeline (F2).
    pub(crate) fn save_and_validate(&mut self) {
        let modified_count = self.agents.iter().filter(|a| a.modified).count();
        if modified_count == 0 {
            self.status_message = Some(("No changes to save - all agent configs are clean".to_owned(), false));
            return;
        }

        // 1. Take VCS snapshot
        drop(take_vcs_snapshot(&self.workspace_root, &format!("snapshot before saving {modified_count} agent configs")));

        // 2. Save modified manifests to disk
        for agent in &mut self.agents {
            if !agent.modified {
                continue;
            }
            if let Err(err) = save_agent_to_disk(agent) {
                self.validation_modal = Some((false, format!("Failed to save {}:\n{err}", agent.file_path.display())));
                return;
            }
        }

        // 3. Run validation
        match run_plugin_validation(&self.workspace_root) {
            Ok(report) => {
                // 4. Install plugin on success
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

        // Reload agents from disk
        self.agents = discover_agents(&self.workspace_root);

        self.validation_modal = None;
        self.status_message = Some((
            format!("Rollback complete: restored {restored} agent manifests to previous state"),
            false,
        ));
    }
}
