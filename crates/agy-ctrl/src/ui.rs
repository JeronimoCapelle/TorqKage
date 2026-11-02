//! Ratatui terminal rendering routines for the agy-ctrl control deck.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};
use crate::app::{ActiveTab, App, FleetFocus};

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
        Span::styled(" ⚙ AGY-CTRL ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(" POLICY DECK ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]))
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    frame.render_widget(title_para, header_chunks[0]);

    // Tabs
    let tab_titles = vec![
        ActiveTab::FleetOverview.title(),
        ActiveTab::ToolCatalog.title(),
        ActiveTab::DirectivesEditor.title(),
    ];
    let selected_tab_idx = match app.active_tab {
        ActiveTab::FleetOverview => 0,
        ActiveTab::ToolCatalog => 1,
        ActiveTab::DirectivesEditor => 2,
    };

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded))
        .select(selected_tab_idx)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    frame.render_widget(tabs, header_chunks[1]);

    // Modified tracker
    let modified_count = app.agents.iter().filter(|a| a.modified).count();
    let mod_span = if modified_count > 0 {
        Span::styled(
            format!(" [● {modified_count} MODIFIED] "),
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" [✔ CLEAN] ", Style::default().fg(Color::Green))
    };

    let mod_para = Paragraph::new(Line::from(vec![mod_span]))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
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

    // Left: Agent List
    let is_agent_focused = app.fleet_focus == FleetFocus::AgentList;
    let agent_border_style = if is_agent_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let agent_items: Vec<ListItem<'_>> = app
        .agents
        .iter()
        .enumerate()
        .map(|(idx, agent)| {
            let is_selected = idx == app.selected_agent_idx;
            let marker = if is_selected { "▶ " } else { "  " };
            let mod_marker = if agent.modified { "*" } else { " " };
            let tools_badge = format!(" [{} tools]", agent.mcp_tools.len());

            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:<13}", agent.name),
                    if is_selected {
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
                Span::styled(mod_marker, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
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

    let agents_list = List::new(agent_items).block(
        Block::default()
            .title(" [Active Agents (Enter/l to Launch in agy)] ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(agent_border_style),
    );
    frame.render_widget(agents_list, main_chunks[0]);

    // Right: Selected Agent Capability Matrix
    if let Some(agent) = app.selected_agent() {
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Metadata banner
                Constraint::Min(8),    // MCP Tools checklist
                Constraint::Length(4), // Tool description preview
            ])
            .split(main_chunks[1]);

        // Top Metadata
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
                Span::styled(&agent.command_execution_policy, Style::default().fg(Color::Magenta)),
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
        ];
        let meta_para = Paragraph::new(meta_lines).block(
            Block::default()
                .title(format!(" [Profile: {}] ", agent.name))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
        frame.render_widget(meta_para, right_chunks[0]);

        // Center: MCP Tool Checklist
        let is_tool_focused = app.fleet_focus == FleetFocus::ToolList;
        let tool_border_style = if is_tool_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Scroll window for tools
        let height = right_chunks.get(1).map_or(0, |c| c.height);
        let visible_tool_count = usize::from(height.saturating_sub(2));
        let scroll_offset = if app.selected_tool_idx >= visible_tool_count {
            app.selected_tool_idx.saturating_sub(visible_tool_count >> 1)
        } else {
            0
        };

        let tool_items: Vec<ListItem<'_>> = app
            .tools
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_tool_count)
            .map(|(idx, tool)| {
                let is_highlighted = idx == app.selected_tool_idx;
                let is_allowed = agent.allows_mcp_tool(&tool.name);

                let check_box = if is_allowed { "[X] " } else { "[ ] " };
                let check_style = if is_allowed {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
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
                            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                    Span::styled(format!(" ({})", tool.action), Style::default().fg(Color::DarkGray)),
                ]);

                let row_style = if is_highlighted {
                    Style::default().bg(Color::Rgb(30, 40, 56))
                } else {
                    Style::default()
                };

                ListItem::new(line).style(row_style)
            })
            .collect();

        let tool_list_widget = List::new(tool_items).block(
            Block::default()
                .title(format!(
                    " [Authorized MCP Tools ({} / {} active) — Space to Toggle] ",
                    agent.mcp_tools.len(),
                    app.tools.len()
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(tool_border_style),
        );
        frame.render_widget(tool_list_widget, right_chunks[1]);

        // Bottom: Tool Description Preview
        let preview_content = app.tools.get(app.selected_tool_idx).map_or_else(
            || vec![Line::from(Span::styled("Select a tool above to view details", Style::default().fg(Color::DarkGray)))],
            |tool| vec![
                Line::from(vec![
                    Span::styled("Tool: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&tool.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("  [Category: {}]", tool.category.label()), Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::styled(&tool.description, Style::default().fg(Color::Gray)),
                ]),
            ],
        );

        let preview_para = Paragraph::new(preview_content).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(preview_para, right_chunks[2]);
    }
}

fn draw_tool_catalog(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), // Filtered Tool List
            Constraint::Percentage(65), // Detailed Policy & Guard Inspector
        ])
        .split(area);

    // Left: Tool List
    let list_items: Vec<ListItem<'_>> = app
        .tools
        .iter()
        .enumerate()
        .map(|(idx, tool)| {
            let is_selected = idx == app.selected_tool_idx;
            let marker = if is_selected { "▶ " } else { "  " };

            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(
                    &tool.name,
                    if is_selected {
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ),
            ]);

            let style = if is_selected {
                Style::default().bg(Color::Rgb(24, 32, 48))
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list_widget = List::new(list_items).block(
        Block::default()
            .title(format!(" [MCP Tool Catalog ({} Tools)] ", app.tools.len()))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list_widget, chunks[0]);

    // Right: Tool Details & Policy Inspector
    if let Some(tool) = app.tools.get(app.selected_tool_idx) {
        let mut detail_lines = vec![
            Line::from(vec![
                Span::styled("Tool Identifier: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&tool.name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
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
            Line::from(Span::styled("─── Execution Parameters ───────────────────────────────", Style::default().fg(Color::DarkGray))),
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
        detail_lines.push(Line::from(Span::styled("─── Composable Security Guards (tools.toml) ────────────", Style::default().fg(Color::DarkGray))));

        let guard_check = |enabled: bool| {
            if enabled {
                Span::styled("[X] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            }
        };

        detail_lines.push(Line::from(vec![
            guard_check(tool.ban_dangerous_files),
            Span::styled("ban_dangerous_files      ", Style::default().fg(Color::White)),
            guard_check(tool.enforce_workspace_bounds),
            Span::styled("enforce_workspace_bounds", Style::default().fg(Color::White)),
        ]));

        detail_lines.push(Line::from(vec![
            guard_check(tool.require_clippy_clean),
            Span::styled("require_clippy_clean     ", Style::default().fg(Color::White)),
            guard_check(tool.require_tests_passed),
            Span::styled("require_tests_passed    ", Style::default().fg(Color::White)),
        ]));

        detail_lines.push(Line::from(vec![
            guard_check(tool.require_git_clean),
            Span::styled("require_git_clean        ", Style::default().fg(Color::White)),
            guard_check(tool.attach_rustc_explain),
            Span::styled("attach_rustc_explain    ", Style::default().fg(Color::White)),
        ]));

        // List agents currently authorized for this tool
        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(Span::styled("─── Authorized Agent Roles ─────────────────────────────", Style::default().fg(Color::DarkGray))));

        let mut authorized_agents = Vec::new();
        for agent in &app.agents {
            if agent.allows_mcp_tool(&tool.name) {
                authorized_agents.push(agent.name.as_str());
            }
        }

        if authorized_agents.is_empty() {
            detail_lines.push(Line::from(Span::styled("  (None — Tool is currently inactive across all agents)", Style::default().fg(Color::Red))));
        } else {
            detail_lines.push(Line::from(vec![
                Span::styled("  Granted to: ", Style::default().fg(Color::DarkGray)),
                Span::styled(authorized_agents.join(", "), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
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

        frame.render_widget(detail_para, chunks[1]);
    }
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

fn draw_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let footer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Shortcuts line
            Constraint::Length(1), // Status message line
        ])
        .split(area);

    // Shortcuts
    let shortcuts = Line::from(vec![
        Span::styled(" [Enter/l Launch] ", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        Span::styled("[Space Toggle] ", Style::default().fg(Color::White)),
        Span::styled("[F1 Help] ", Style::default().fg(Color::Yellow)),
        Span::styled("[F2 Save & Validate] ", Style::default().fg(Color::Green)),
        Span::styled("[F4 Rollback] ", Style::default().fg(Color::Red)),
        Span::styled("[Tab Switch View] ", Style::default().fg(Color::Cyan)),
        Span::styled("[j/k Navigate] ", Style::default().fg(Color::DarkGray)),
        Span::styled("[q Quit] ", Style::default().fg(Color::DarkGray)),
    ]);
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
        Line::from(Span::styled("AGY-CTRL KEYBOARD SHORTCUTS & NAVIGATION", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter / l       ", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
            Span::styled("Run pre-flight checks & launch interactive agy session with selected agent", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Tab / BackTab   ", Style::default().fg(Color::Cyan)),
            Span::styled("Switch between Fleet Matrix, Tool Catalog, and Directives", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  ↑ / ↓ (or j / k)", Style::default().fg(Color::Cyan)),
            Span::styled("Navigate agents or tools in the active list", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Space           ", Style::default().fg(Color::Cyan)),
            Span::styled("Toggle tool authorization on/off for selected agent", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  h / l (or ← / →)", Style::default().fg(Color::Cyan)),
            Span::styled("Switch focus between Agent List and Tool List", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  F2 (or s)       ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Atomic Save & Pre-flight Validation (runs agy plugin validate)", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  F4 (or r)       ", Style::default().fg(Color::Red)),
            Span::styled("Atomic Rollback: revert unsaved changes to disk/VCS state", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Esc / F1        ", Style::default().fg(Color::Cyan)),
            Span::styled("Close modal or help dialog", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  q               ", Style::default().fg(Color::Cyan)),
            Span::styled("Exit agy-ctrl", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press Esc or Enter to close this help dialog.", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .title(" [Help & Documentation] ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow));

    let modal = Paragraph::new(help_text).block(block).wrap(Wrap { trim: true });
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
                Line::from(Span::styled(l, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)))
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

    let modal = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(modal, modal_area);
}

/// Helper function to create a centered rectangular area for modals.
#[allow(clippy::arithmetic_side_effects, reason = "Modal centered rectangle percentage layout")]
#[allow(clippy::indexing_slicing, reason = "Layout split returns array matching constraints length")]
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
