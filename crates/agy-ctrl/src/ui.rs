//! Ratatui terminal rendering routines for the agy-ctrl control deck.

#![allow(
    clippy::indexing_slicing,
    reason = "Ratatui layout splits produce fixed-length chunk slices"
)]
#![allow(
    clippy::too_many_lines,
    reason = "Terminal UI rendering pipelines for multi-pane layouts"
)]

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::{
    app::{ActiveTab, App, FleetFocus},
    model::AgentManifest,
};

/// Main draw dispatcher called on every terminal frame.
pub(crate) fn draw(frame: &mut Frame<'_>, app: &App) {
    let size = frame.area();

    // 1. Overall vertical layout: Header, Main View, Footer Status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header & Tab bar
            Constraint::Min(10),   // Active Tab View
            Constraint::Length(3), // Footer status bar
        ])
        .split(size);

    draw_header(frame, app, chunks[0]);

    match app.active_tab {
        ActiveTab::FleetOverview => draw_fleet_matrix(frame, app, chunks[1]),
        ActiveTab::ToolCatalog => draw_tool_catalog(frame, app, chunks[1]),
        ActiveTab::DirectivesEditor => draw_directives_editor(frame, app, chunks[1]),
        ActiveTab::Manual => draw_manual_view(frame, app, chunks[1]),
    }

    draw_footer(frame, app, chunks[2]);

    // Modals drawn on top of the frame if triggered
    if app.show_help {
        draw_help_modal(frame, size);
    } else if let Some((is_success, ref message)) = app.validation_modal {
        draw_validation_modal(frame, size, is_success, message);
    }
}

fn draw_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(32),
            Constraint::Min(40),
            Constraint::Length(24),
        ])
        .split(area);

    // Title
    let title_para = Paragraph::new(Line::from(vec![
        Span::styled(
            " ⚙ AGY-CTRL ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " POLICY DECK ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    frame.render_widget(title_para, header_chunks[0]);

    // Tabs
    let tab_titles = vec![
        ActiveTab::FleetOverview.title(),
        ActiveTab::ToolCatalog.title(),
        ActiveTab::DirectivesEditor.title(),
        ActiveTab::Manual.title(),
    ];
    let selected_tab_idx = match app.active_tab {
        ActiveTab::FleetOverview => 0,
        ActiveTab::ToolCatalog => 1,
        ActiveTab::DirectivesEditor => 2,
        ActiveTab::Manual => 3,
    };

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(selected_tab_idx)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, header_chunks[1]);

    // Modified tracker & Sandbox state
    let modified_count = app
        .agents
        .iter()
        .filter(|a| a.modified)
        .count()
        .saturating_add(app.tools.iter().filter(|t| t.modified).count());
    let mod_span = if modified_count > 0 {
        Span::styled(
            format!(" [● {modified_count} MODIFIED] "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" [✔ CLEAN] ", Style::default().fg(Color::Green))
    };

    let sandbox_span = if app.sandbox_enabled() {
        Span::styled(
            " [SANDBOX: ON] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" [SANDBOX: OFF] ", Style::default().fg(Color::DarkGray))
    };

    let iso_span = if app.isolated_workspace() {
        Span::styled(
            " [ISO: ON] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" [ISO: OFF] ", Style::default().fg(Color::DarkGray))
    };

    let mod_para = Paragraph::new(Line::from(vec![mod_span, sandbox_span, iso_span]))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
    frame.render_widget(mod_para, header_chunks[2]);
}

fn draw_fleet_matrix(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(28), // Agent List
            Constraint::Percentage(72), // Capability Matrix & Tool Toggles
        ])
        .split(area);

    draw_fleet_agent_list(frame, app, main_chunks[0]);
    draw_fleet_agent_details(frame, app, main_chunks[1]);
}

fn draw_fleet_agent_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let is_agent_focused = app.fleet_focus == FleetFocus::AgentList;
    let agent_border_style = if is_agent_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let agent_viewport = usize::from(area.height.saturating_sub(2));
    let (agent_offset, agent_visible) =
        compute_scroll_window(app.selected_agent_idx, app.agents.len(), agent_viewport);

    let agent_items: Vec<ListItem<'_>> = app
        .agents
        .iter()
        .enumerate()
        .skip(agent_offset)
        .take(agent_visible)
        .map(|(idx, agent)| {
            let is_selected = idx == app.selected_agent_idx;
            let marker = if is_selected { "▶ " } else { "  " };
            let mod_marker = if agent.modified { "*" } else { " " };
            let policy_badge = match (
                agent.permission_mode.as_str(),
                agent.command_execution_policy.as_str(),
            ) {
                ("plan", _) => Span::styled(" [PLAN]", Style::default().fg(Color::Cyan)),
                (_, "ask" | "prompt") => Span::styled(" [ASK]", Style::default().fg(Color::Yellow)),
                _ => Span::styled(" [AUTO]", Style::default().fg(Color::Green)),
            };
            let tools_badge = format!(" [{} tools]", agent.mcp_tools.len());

            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:<11}", agent.name),
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
                Span::styled(
                    mod_marker,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                policy_badge,
                Span::styled(tools_badge, Style::default().fg(Color::DarkGray)),
            ]);

            let style = if is_selected {
                Style::default().bg(Color::Rgb(24, 32, 48))
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let agent_pos = app.selected_agent_idx.saturating_add(1);
    let total_agents = app.agents.len();
    let agents_list = List::new(agent_items).block(
        Block::default()
            .title(format!(
                " [Active Agents: {agent_pos}/{total_agents} (Enter/l to Launch in agy)] "
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(agent_border_style),
    );
    frame.render_widget(agents_list, area);
}

fn draw_fleet_agent_details(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(agent) = app.selected_agent() else {
        return;
    };

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Metadata banner with session resumption & circuit breaker
            Constraint::Min(8),    // MCP Tools checklist
            Constraint::Length(4), // Tool description preview
        ])
        .split(area);

    draw_agent_metadata_panel(frame, app, agent, right_chunks[0]);
    draw_agent_tool_checklist(frame, app, agent, right_chunks[1]);
    draw_agent_tool_preview(frame, app, right_chunks[2]);
}

fn draw_agent_metadata_panel(frame: &mut Frame<'_>, app: &App, agent: &AgentManifest, area: Rect) {
    let (session_span, action_span) = app.active_agent_session.as_ref().map_or_else(
        || {
            (
                Span::styled(
                    "None (no prior session found for workspace)",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    "[Enter/n: Start Fresh Session]",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            )
        },
        |sess| {
            (
                Span::styled(
                    format!(
                        "Resumable (conv: {}, {} steps)",
                        sess.short_id(),
                        sess.step_count
                    ),
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "[Enter/r: Resume]  [n: New Session]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            )
        },
    );

    let cb_span = match app.circuit_breaker_status.as_ref() {
        Some(cb) if cb.tripped => Span::styled(
            format!(
                "🛑 TRIPPED ({}/{} strikes - {})",
                cb.consecutive_failures,
                cb.max_strikes,
                cb.last_error_signature.as_deref().unwrap_or("fail")
            ),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Some(cb) if cb.consecutive_failures > 0 => Span::styled(
            format!("⚠ {}/{} strikes", cb.consecutive_failures, cb.max_strikes),
            Style::default().fg(Color::Yellow),
        ),
        _ => Span::styled("✔ OK (0/2 strikes)", Style::default().fg(Color::Green)),
    };

    let meta_lines = vec![
        Line::from(vec![
            Span::styled("Role: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&agent.description, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Effort: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&agent.effort, Style::default().fg(Color::Cyan)),
            Span::styled(" | Mode: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&agent.permission_mode, Style::default().fg(Color::Yellow)),
            Span::styled(" | Policy: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &agent.command_execution_policy,
                Style::default().fg(Color::Magenta),
            ),
            Span::styled(" | Native Tools: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if agent.native_tools.is_empty() {
                    "[] (Air-gapped)".to_owned()
                } else {
                    agent.native_tools.join(", ")
                },
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Session: ", Style::default().fg(Color::DarkGray)),
            session_span,
            Span::styled(" | Launch: ", Style::default().fg(Color::DarkGray)),
            action_span,
        ]),
        Line::from(vec![
            Span::styled("Workspace: ", Style::default().fg(Color::DarkGray)),
            if app.isolated_workspace() {
                Span::styled(
                    format!("Isolated (.agy/workspaces/{})", agent.name),
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("Direct (@)", Style::default().fg(Color::DarkGray))
            },
            Span::styled(
                " [w: toggle] | Circuit Breaker: ",
                Style::default().fg(Color::DarkGray),
            ),
            cb_span,
            Span::styled(" [k: reset]", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let meta_para = Paragraph::new(meta_lines).block(
        Block::default()
            .title(format!(" [Profile: {}] ", agent.name))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    frame.render_widget(meta_para, area);
}

fn draw_agent_tool_checklist(frame: &mut Frame<'_>, app: &App, agent: &AgentManifest, area: Rect) {
    let is_tool_focused = app.fleet_focus == FleetFocus::ToolList;
    let tool_border_style = if is_tool_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let viewport_height = usize::from(area.height.saturating_sub(2));
    let (scroll_offset, visible_count) =
        compute_scroll_window(app.selected_tool_idx, app.tools.len(), viewport_height);

    let tool_items: Vec<ListItem<'_>> = app
        .tools
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_count)
        .map(|(idx, tool)| {
            let is_highlighted = idx == app.selected_tool_idx;
            let is_allowed = agent.allows_mcp_tool(&tool.name);

            let check_box = if is_allowed { "[X] " } else { "[ ] " };
            let check_style = if is_allowed {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let marker = if is_highlighted { "▶ " } else { "  " };

            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(check_box, check_style),
                Span::styled(
                    format!("{:<26}", tool.name),
                    if is_allowed {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    format!(" ({})", tool.action),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            let row_style = if is_highlighted {
                Style::default().bg(Color::Rgb(30, 40, 56))
            } else {
                Style::default()
            };

            ListItem::new(line).style(row_style)
        })
        .collect();

    let tool_pos = app.selected_tool_idx.saturating_add(1);
    let total_tools = app.tools.len();
    let tool_list_widget = List::new(tool_items).block(
        Block::default()
            .title(format!(
                " [Authorized MCP Tools: {tool_pos}/{total_tools} ({} active) — Space to Toggle] ",
                agent.mcp_tools.len()
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(tool_border_style),
    );
    frame.render_widget(tool_list_widget, area);
}

fn draw_agent_tool_preview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let preview_content = app.tools.get(app.selected_tool_idx).map_or_else(
        || {
            vec![Line::from(Span::styled(
                "Select a tool above to view details",
                Style::default().fg(Color::DarkGray),
            ))]
        },
        |tool| {
            vec![
                Line::from(vec![
                    Span::styled("Tool: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        &tool.name,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  [Category: {}]", tool.category.label()),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
                Line::from(vec![Span::styled(
                    &tool.description,
                    Style::default().fg(Color::Gray),
                )]),
            ]
        },
    );

    let preview_para = Paragraph::new(preview_content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(preview_para, area);
}

fn draw_tool_catalog(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), // Filtered Tool List
            Constraint::Percentage(65), // Detailed Policy & Guard Inspector
        ])
        .split(area);

    draw_tool_catalog_list(frame, app, chunks[0]);
    draw_tool_catalog_details(frame, app, chunks[1]);
}

fn draw_tool_catalog_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let viewport_height = usize::from(area.height.saturating_sub(2));
    let (scroll_offset, visible_count) =
        compute_scroll_window(app.selected_tool_idx, app.tools.len(), viewport_height);

    let list_items: Vec<ListItem<'_>> = app
        .tools
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_count)
        .map(|(idx, tool)| {
            let is_selected = idx == app.selected_tool_idx;
            let marker = if is_selected { "▶ " } else { "  " };

            let sandbox_badge = if tool.enforce_sandbox() {
                Span::styled(
                    " [SANDBOX]",
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("")
            };

            let gate_badge = if tool.require_permission_gate() {
                Span::styled(
                    " [ASK]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(" [ALLOW]", Style::default().fg(Color::DarkGray))
            };

            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(
                    &tool.name,
                    if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
                sandbox_badge,
                gate_badge,
            ]);

            let style = if is_selected {
                Style::default().bg(Color::Rgb(24, 32, 48))
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let current_pos = app.selected_tool_idx.saturating_add(1);
    let total = app.tools.len();
    let list_widget = List::new(list_items).block(
        Block::default()
            .title(format!(
                " [MCP Tool Catalog: {current_pos}/{total} (↑/↓ to scroll)] "
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list_widget, area);
}

fn build_tool_guard_lines(tool: &crate::model::ToolItem) -> Vec<Line<'static>> {
    let guard_check = |enabled: bool| {
        if enabled {
            Span::styled(
                "[X] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
        }
    };

    let gates_label = if tool.required_gates().is_empty() {
        "none".to_owned()
    } else {
        tool.required_gates().join(", ")
    };

    vec![
        Line::from(vec![
            guard_check(tool.ban_dangerous_files()),
            Span::styled(
                "ban_dangerous_files      ",
                Style::default().fg(Color::White),
            ),
            guard_check(tool.enforce_workspace_bounds()),
            Span::styled(
                "enforce_workspace_bounds",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            guard_check(tool.mutating()),
            Span::styled(
                "mutating (modifies disk) ",
                Style::default().fg(Color::White),
            ),
            guard_check(tool.tracks_circuit_breaker()),
            Span::styled(
                "tracks_circuit_breaker  ",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            guard_check(tool.enforce_sandbox()),
            Span::styled(
                "enforce_sandbox (bwrap)  ",
                Style::default().fg(Color::LightCyan),
            ),
            guard_check(tool.emit_truncation_notice()),
            Span::styled(
                "emit_truncation_notice  ",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  require_gates: ", Style::default().fg(Color::Yellow)),
            Span::styled(gates_label, Style::default().fg(Color::White)),
        ]),
    ]
}

fn draw_tool_catalog_details(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(tool) = app.tools.get(app.selected_tool_idx) else {
        return;
    };

    let mut detail_lines = vec![
        Line::from(vec![
            Span::styled("Tool Identifier: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &tool.name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Description:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(&tool.description, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Action Engine:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(&tool.action, Style::default().fg(Color::Yellow)),
            Span::styled("  [Category: ", Style::default().fg(Color::DarkGray)),
            Span::styled(tool.category.label(), Style::default().fg(Color::Magenta)),
            Span::styled("]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "─── Execution Parameters ───────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if let Some(ref cmd) = tool.command {
        detail_lines.push(Line::from(vec![
            Span::styled("Command Argv:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{cmd:?}"), Style::default().fg(Color::Green)),
        ]));
    }

    if let Some(ref dirs) = tool.allowed_directories {
        detail_lines.push(Line::from(vec![
            Span::styled("Allowed Dirs:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(dirs.join(", "), Style::default().fg(Color::Yellow)),
        ]));
    }

    if let Some(ref cap) = tool.output_capture {
        detail_lines.push(Line::from(vec![
            Span::styled("Output Capture:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(cap, Style::default().fg(Color::White)),
        ]));
    }

    if let Some(timeout) = tool.timeout_seconds {
        detail_lines.push(Line::from(vec![
            Span::styled("Timeout:         ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{timeout}s"), Style::default().fg(Color::White)),
        ]));
    }

    detail_lines.push(Line::from(""));
    detail_lines.push(Line::from(Span::styled(
        "─── Composable Security Guards (tools.toml) ────────────",
        Style::default().fg(Color::DarkGray),
    )));
    detail_lines.extend(build_tool_guard_lines(tool));

    // List agents currently authorized for this tool
    detail_lines.push(Line::from(""));
    detail_lines.push(Line::from(Span::styled(
        "─── Authorized Agent Roles ─────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));

    let mut authorized_agents = Vec::new();
    for agent in &app.agents {
        if agent.allows_mcp_tool(&tool.name) {
            authorized_agents.push(agent.name.as_str());
        }
    }

    if authorized_agents.is_empty() {
        detail_lines.push(Line::from(Span::styled(
            "  (None — Tool is currently inactive across all agents)",
            Style::default().fg(Color::Red),
        )));
    } else {
        detail_lines.push(Line::from(vec![
            Span::styled("  Granted to: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                authorized_agents.join(", "),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    let detail_para = Paragraph::new(detail_lines)
        .block(
            Block::default()
                .title(" [Tool Inspection & Policy Matrix] ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(detail_para, area);
}

fn draw_directives_editor(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if let Some(agent) = app.selected_agent() {
        let lines: Vec<Line<'_>> = agent
            .directives
            .lines()
            .skip(usize::from(app.directives_scroll))
            .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::White))))
            .collect();

        let editor_widget = Paragraph::new(lines).block(
            Block::default()
                .title(format!(
                    " [Directives & System Prompt: {} (Scroll: ↑/↓, PgUp/PgDn)] ",
                    agent.name
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(editor_widget, area);
    }
}

fn draw_manual_view(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let content_lines = crate::manual::controls_content();
    let content_block = Block::default()
        .title(" [Controls & Keybindings Reference] (Scroll: j/k, PgUp/PgDn, Esc/Enter to close) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    let content_para = Paragraph::new(content_lines)
        .block(content_block)
        .wrap(Wrap { trim: false })
        .scroll((app.manual_scroll, 0));

    frame.render_widget(content_para, area);
}

fn draw_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let footer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Shortcuts line
            Constraint::Length(1), // Status message line
        ])
        .split(area);

    // Dynamic context-sensitive shortcuts
    let shortcuts = match app.active_tab {
        ActiveTab::FleetOverview => Line::from(vec![
            Span::styled(
                " [Enter Launch] ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "[n New] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("[w Iso] ", Style::default().fg(Color::LightCyan)),
            Span::styled("[b Sandbox] ", Style::default().fg(Color::Yellow)),
            Span::styled("[p Policy] ", Style::default().fg(Color::Magenta)),
            Span::styled("[c Reset CB] ", Style::default().fg(Color::LightRed)),
            Span::styled(
                "[i Install/Health] ",
                Style::default().fg(Color::LightMagenta),
            ),
            Span::styled("[Space Toggle] ", Style::default().fg(Color::White)),
            Span::styled("[F2 Save] ", Style::default().fg(Color::Green)),
            Span::styled("[?] Manual ", Style::default().fg(Color::Yellow)),
            Span::styled("[q Quit] ", Style::default().fg(Color::DarkGray)),
        ]),
        ActiveTab::ToolCatalog => Line::from(vec![
            Span::styled(
                " [j/k Navigate] ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "[g Toggle Gate (Ask/Allow)] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "[i Install/Health] ",
                Style::default().fg(Color::LightMagenta),
            ),
            Span::styled("[F2 Save] ", Style::default().fg(Color::Green)),
            Span::styled("[F4 Undo] ", Style::default().fg(Color::Red)),
            Span::styled("[?] Manual ", Style::default().fg(Color::Yellow)),
            Span::styled("[q Quit] ", Style::default().fg(Color::DarkGray)),
        ]),
        ActiveTab::DirectivesEditor => Line::from(vec![
            Span::styled(
                " [j/k Scroll Prompt] ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("[PageUp/Dn] ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "[i Install/Health] ",
                Style::default().fg(Color::LightMagenta),
            ),
            Span::styled("[F2 Save] ", Style::default().fg(Color::Green)),
            Span::styled("[F4 Undo] ", Style::default().fg(Color::Red)),
            Span::styled("[?] Manual ", Style::default().fg(Color::Yellow)),
            Span::styled("[q Quit] ", Style::default().fg(Color::DarkGray)),
        ]),
        ActiveTab::Manual => Line::from(vec![
            Span::styled(
                " [ [ / ] Sections] ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("[j/k Scroll] ", Style::default().fg(Color::Cyan)),
            Span::styled("[PageUp/Dn] ", Style::default().fg(Color::LightCyan)),
            Span::styled("[g Top] ", Style::default().fg(Color::White)),
            Span::styled(
                "[i Install/Health] ",
                Style::default().fg(Color::LightMagenta),
            ),
            Span::styled("[Esc Deck] ", Style::default().fg(Color::Yellow)),
            Span::styled("[q Quit] ", Style::default().fg(Color::DarkGray)),
        ]),
    };
    frame.render_widget(Paragraph::new(shortcuts), footer_chunks[0]);

    // Live status message
    if let Some((ref msg, is_error)) = app.status_message {
        let style = if is_error {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };
        frame.render_widget(Paragraph::new(Span::styled(msg, style)), footer_chunks[1]);
    }
}

fn draw_help_modal(frame: &mut Frame<'_>, area: Rect) {
    let modal_area = centered_rect(65, 55, area);
    frame.render_widget(Clear, modal_area);

    let help_text = vec![
        Line::from(Span::styled(
            "AGY-CTRL KEYBOARD SHORTCUTS & NAVIGATION",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Enter / l       ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Run pre-flight checks & launch interactive agy session with selected agent",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Tab / BackTab   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Switch between Fleet Matrix, Tool Catalog, and Directives",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ↑ / ↓ (or j / k)", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Navigate agents or tools in the active list",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Space           ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Toggle tool authorization on/off for selected agent",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  h / l (or ← / →)", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Switch focus between Agent List and Tool List",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  F2 (or s)       ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Atomic Save & Pre-flight Validation (runs agy plugin validate)",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  F4 (or r)       ", Style::default().fg(Color::Red)),
            Span::styled(
                "Atomic Rollback: revert unsaved changes to disk/VCS state",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  w               ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Toggle Jujutsu workspace isolation (.agy/workspaces/<agent>)",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  k               ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Reset 2-strike compiler circuit breaker to clean state",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Esc / F1        ", Style::default().fg(Color::Cyan)),
            Span::styled(
                "Close modal or help dialog",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  q               ", Style::default().fg(Color::Cyan)),
            Span::styled("Exit agy-ctrl", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc or Enter to close this help dialog.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" [Help & Documentation] ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow));

    let modal = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: true });
    frame.render_widget(modal, modal_area);
}

fn draw_validation_modal(frame: &mut Frame<'_>, area: Rect, is_success: bool, message: &str) {
    let modal_area = centered_rect(75, 60, area);
    frame.render_widget(Clear, modal_area);

    let border_color = if is_success { Color::Green } else { Color::Red };
    let title = if is_success {
        " [✔ Pre-Flight Validation Passed] "
    } else {
        " [🛑 Pre-Flight Validation Failed — Error Diagnostics] "
    };

    let lines: Vec<Line<'_>> = message
        .lines()
        .map(|l| {
            if l.contains('✔') || l.contains("passed") {
                Line::from(Span::styled(l, Style::default().fg(Color::Green)))
            } else if l.contains('❌') || l.contains("failed") || l.contains("Error") {
                Line::from(Span::styled(
                    l,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(l, Style::default().fg(Color::White)))
            }
        })
        .collect();

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(border_color));

    let modal = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(modal, modal_area);
}

/// Calculates the window slice `(scroll_offset, visible_count)` to keep `selected_idx`
/// centered and fully visible within a viewport of height `viewport_height`.
const fn compute_scroll_window(
    selected_idx: usize,
    total_items: usize,
    viewport_height: usize,
) -> (usize, usize) {
    if viewport_height == 0 || total_items == 0 {
        return (0, 0);
    }
    let visible_count = if viewport_height < total_items {
        viewport_height
    } else {
        total_items
    };

    let scroll_offset = if selected_idx >= visible_count {
        let half = visible_count >> 1;
        let offset = selected_idx.saturating_sub(half);
        if offset.saturating_add(visible_count) > total_items {
            total_items.saturating_sub(visible_count)
        } else {
            offset
        }
    } else {
        0
    };
    (scroll_offset, visible_count)
}

/// Helper function to create a centered rectangular area for modals.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "Modal centered rectangle percentage layout"
)]
#[allow(
    clippy::indexing_slicing,
    reason = "Layout split returns array matching constraints length"
)]
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let pad_y = 100_u16.saturating_sub(percent_y) / 2;
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(pad_y),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(pad_y),
        ])
        .split(r);

    let pad_x = 100_u16.saturating_sub(percent_x) / 2;
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(pad_x),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(pad_x),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_scroll_window() {
        // Empty
        assert_eq!(compute_scroll_window(0, 0, 10), (0, 0));
        assert_eq!(compute_scroll_window(0, 50, 0), (0, 0));

        // Total items smaller than viewport
        assert_eq!(compute_scroll_window(3, 9, 20), (0, 9));

        // Start of list
        let (start_offset, start_count) = compute_scroll_window(0, 64, 20);
        assert_eq!((start_offset, start_count), (0, 20));

        // Middle of list (cursor centered)
        let (mid_offset, mid_count) = compute_scroll_window(25, 64, 20);
        assert_eq!((mid_offset, mid_count), (15, 20));
        assert!(25 >= mid_offset && 25 < mid_offset.saturating_add(mid_count));

        // End of list (cursor clamped to end)
        let (end_offset, end_count) = compute_scroll_window(63, 64, 20);
        assert_eq!((end_offset, end_count), (44, 20));
        assert_eq!(end_offset.saturating_add(end_count), 64);
        assert!(63 >= end_offset && 63 < end_offset.saturating_add(end_count));
    }
}
