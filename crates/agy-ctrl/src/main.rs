//! # agy-ctrl
//!
//! Offline Ratatui terminal configuration deck and CLI management utility for `agy-suite`.

#![allow(clippy::print_stdout, reason = "CLI summary flags emit output to stdout")]
#![allow(clippy::print_stderr, reason = "CLI error diagnostics emit output to stderr")]
#![allow(clippy::indexing_slicing, reason = "Ratatui layout splits produce fixed-length chunk slices")]
#![allow(clippy::too_many_lines, reason = "Ratatui rendering pipelines for multi-pane terminal views")]
#![allow(clippy::format_push_string, reason = "Convenient markdown and diagnostic generation")]
#![allow(clippy::struct_excessive_bools, reason = "Mirrors declarative tools.toml and manifest schemas")]

mod app;
mod model;
mod summary;
mod ui;
mod vcs;

use std::io::{self, stdout};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use agy_common::WorkspaceResolver;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use crate::app::{ActiveTab, App, FleetFocus};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str);

    let workspace_root = WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."));

    match command {
        Some("--status" | "status") => {
            let app = match App::new(workspace_root) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("🛑 [agy-ctrl] Failed to load configuration: {e}");
                    return ExitCode::FAILURE;
                }
            };
            summary::print_fleet_status(&app.agents, &app.tools);
            ExitCode::SUCCESS
        }
        Some("--validate" | "validate") => {
            if let Err(e) = summary::print_validation_report(&workspace_root) {
                eprintln!("🛑 [agy-ctrl] Validation failed: {e}");
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Some("--help" | "-h" | "help") => {
            print_cli_usage();
            ExitCode::SUCCESS
        }
        _ => {
            // Interactive Ratatui TUI Mode
            if let Err(err) = run_tui(workspace_root) {
                eprintln!("🛑 [agy-ctrl] TUI error: {err}");
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
    }
}

fn print_cli_usage() {
    println!(
        "agy-ctrl: Offline Ratatui terminal configuration deck for agy-suite\n\
         \n\
         USAGE:\n\
           agy-ctrl [OPTIONS]\n\
         \n\
         OPTIONS:\n\
           --status, status      Print ASCII agent fleet status & tool distribution matrix\n\
           --validate, validate  Run pre-flight validation on agy-suite plugin and tools\n\
           --help, -h            Show this help reference\n\
         \n\
         INTERACTIVE TUI:\n\
           Run without arguments in an interactive terminal to launch the visual policy deck.\n"
    );
}

fn run_tui(workspace_root: PathBuf) -> Result<(), String> {
    let mut app = App::new(workspace_root)?;

    // Set up terminal panic hook to guarantee terminal cleanup
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        drop(disable_raw_mode());
        drop(execute!(stdout(), LeaveAlternateScreen));
        default_hook(panic_info);
    }));

    // Initialize terminal
    enable_raw_mode().map_err(|e| format!("Failed to enable raw mode: {e}"))?;
    let mut stdout_handle = stdout();
    execute!(stdout_handle, EnterAlternateScreen)
        .map_err(|e| format!("Failed to enter alternate screen: {e}"))?;

    let backend = CrosstermBackend::new(stdout_handle);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| format!("Failed to initialize terminal: {e}"))?;

    let res = event_loop(&mut terminal, &mut app);

    // Restore terminal
    drop(disable_raw_mode());
    drop(execute!(terminal.backend_mut(), LeaveAlternateScreen));
    drop(terminal.show_cursor());

    res
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), String> {
    while !app.should_quit {
        terminal
            .draw(|f| ui::draw(f, app))
            .map_err(|e| format!("Draw error: {e}"))?;

        if event::poll(Duration::from_millis(100)).map_err(|e| format!("Poll error: {e}"))? {
            match event::read().map_err(|e| format!("Read error: {e}"))? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key_event(app, key.code, key.modifiers);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn handle_key_event(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // 1. Modals capture keys first
    if app.show_help {
        if matches!(code, KeyCode::Esc | KeyCode::Enter | KeyCode::F(1) | KeyCode::Char('q')) {
            app.show_help = false;
        }
        return;
    }

    if app.validation_modal.is_some() {
        match code {
            KeyCode::Esc | KeyCode::Enter => {
                app.validation_modal = None;
            }
            KeyCode::F(4) | KeyCode::Char('r') => {
                app.rollback();
            }
            _ => {}
        }
        return;
    }

    // 2. Global Shortcuts
    match code {
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            return;
        }
        KeyCode::Char('q') => {
            app.should_quit = true;
            return;
        }
        KeyCode::F(1) | KeyCode::Char('?') => {
            app.show_help = true;
            return;
        }
        KeyCode::F(2) | KeyCode::Char('s') => {
            app.save_and_validate();
            return;
        }
        KeyCode::F(4) | KeyCode::Char('r') => {
            app.rollback();
            return;
        }
        KeyCode::Tab => {
            app.next_tab();
            return;
        }
        KeyCode::BackTab => {
            app.prev_tab();
            return;
        }
        KeyCode::Char('1') => {
            app.active_tab = ActiveTab::FleetOverview;
            return;
        }
        KeyCode::Char('2') => {
            app.active_tab = ActiveTab::ToolCatalog;
            return;
        }
        KeyCode::Char('3') => {
            app.active_tab = ActiveTab::DirectivesEditor;
            return;
        }
        _ => {}
    }

    // 3. Tab-Specific Navigation
    match app.active_tab {
        ActiveTab::FleetOverview => handle_fleet_overview_keys(app, code),
        ActiveTab::ToolCatalog => handle_tool_catalog_keys(app, code),
        ActiveTab::DirectivesEditor => handle_directives_editor_keys(app, code),
    }
}

fn handle_fleet_overview_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') => {
            app.toggle_fleet_focus();
        }
        KeyCode::Up | KeyCode::Char('k') => match app.fleet_focus {
            FleetFocus::AgentList => app.prev_agent(),
            FleetFocus::ToolList => app.prev_tool(),
        },
        KeyCode::Down | KeyCode::Char('j') => match app.fleet_focus {
            FleetFocus::AgentList => app.next_agent(),
            FleetFocus::ToolList => app.next_tool(),
        },
        KeyCode::Char(' ') | KeyCode::Enter => {
            app.toggle_current_tool();
        }
        _ => {}
    }
}

fn handle_tool_catalog_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => app.prev_tool(),
        KeyCode::Down | KeyCode::Char('j') => app.next_tool(),
        KeyCode::PageUp => {
            for _ in 0..10 {
                app.prev_tool();
            }
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                app.next_tool();
            }
        }
        _ => {}
    }
}

const fn handle_directives_editor_keys(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.directives_scroll = app.directives_scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.directives_scroll = app.directives_scroll.saturating_add(1);
        }
        KeyCode::PageUp => {
            app.directives_scroll = app.directives_scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            app.directives_scroll = app.directives_scroll.saturating_add(10);
        }
        _ => {}
    }
}
