//! # manual
//!
//! Interactive controls, keybindings, and shortcuts reference for `tkg`.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

fn h1(text: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        format!(" === {text} === "),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ))
}

fn h2(text: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        format!(" --- {text} ---"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn key(k: &'static str, desc: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {k:<20}"),
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc, Style::default().fg(Color::White)),
    ])
}

fn empty() -> Line<'static> {
    Line::from("")
}

/// Returns the formatted controls and shortcuts reference lines for display.
#[must_use]
pub(crate) fn controls_content() -> Vec<Line<'static>> {
    vec![
        h1("TKG CONTROLS & SHORTCUT REFERENCE"),
        empty(),
        h2("Global Navigation"),
        key(
            "1 / 2 / 3 / 4",
            "Switch views: [1] Fleet, [2] Tools, [3] Directives, [4] Controls & Help",
        ),
        key("Tab / BackTab", "Cycle sequentially through views"),
        key("? / F1", "Toggle this Controls & Help overlay"),
        key("q / Ctrl+C", "Quit tkg control deck"),
        empty(),
        h2("Session Management & Launching"),
        key(
            "Enter / r",
            "Resume active agent session (or launch new session if none exists)",
        ),
        key(
            "n",
            "Force launch a fresh, unresumed session for selected agent",
        ),
        key(
            "b / F8",
            "Toggle outer-loop Bubblewrap sandbox isolation for launched sessions",
        ),
        key("w", "Toggle isolated workspace (.tkg/workspaces/<agent>)"),
        key(
            "p",
            "Cycle agent permission policy: Autonomous -> Supervised -> Planning",
        ),
        key(
            "c / Shift+K",
            "Reset failure circuit breaker (clears consecutive failure counter)",
        ),
        key(
            "i",
            "Run in-deck plugin validation, installer, and environment diagnostics",
        ),
        empty(),
        h2("Fleet & Tool Configuration"),
        key(
            "j / k (or ↓ / ↑)",
            "Navigate agents, tools, or scroll content",
        ),
        key(
            "h / l (or ← / →)",
            "Toggle focus between Agent List (left) and Tool List (right)",
        ),
        key(
            "Space",
            "Toggle highlighted tool permission for selected agent",
        ),
        key(
            "g",
            "Toggle tool permission prompt (ASK operator vs ALLOW autonomous)",
        ),
        key(
            "F2 / S",
            "Atomic Save & Validate: snapshot VCS, validate plugin, and install",
        ),
        key(
            "u / U / F4",
            "Atomic Rollback: revert unsaved modifications to original disk/VCS state",
        ),
        empty(),
        h2("Scrolling & Viewport"),
        key("PageUp / PageDn", "Scroll viewport by 10 lines"),
        key("Home / g", "Scroll to top"),
        key("Esc / Enter", "Close help view and return to previous tab"),
        empty(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controls_content_not_empty() {
        let lines = controls_content();
        assert_ne!(lines, []);
    }
}
