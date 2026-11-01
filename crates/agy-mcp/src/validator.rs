//! Validation engine verifying tool specifications and host binary prerequisites.

use crate::models::{ToolAction, ToolDefinition};

/// Diagnostic summary produced when inspecting the declarative toolset at boot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ValidationReport {
    pub(crate) total_tools: usize,
    pub(crate) verified_tools: usize,
    pub(crate) missing_binaries: Vec<(String, String)>,
    pub(crate) warnings: Vec<String>,
}

impl ValidationReport {
    /// Formats a concise startup diagnostic line for daemon stderr logging.
    #[must_use]
    pub(crate) fn summary(&self) -> String {
        if self.missing_binaries.is_empty() {
            format!(
                "[agy-mcp] All {} declarative tools verified (prerequisite binaries active)",
                self.total_tools
            )
        } else {
            let missing_names: Vec<String> = self
                .missing_binaries
                .iter()
                .map(|(tool, bin)| format!("{tool} ({bin})"))
                .take(4)
                .collect();

            let sample = missing_names.join(", ");
            let suffix = if self.missing_binaries.len() > 4 {
                format!(" +{} more", self.missing_binaries.len().saturating_sub(4))
            } else {
                String::new()
            };

            format!(
                "[agy-mcp] Loaded {} tools: {} operational, {} with missing binaries [{sample}{suffix}]",
                self.total_tools, self.verified_tools, self.missing_binaries.len()
            )
        }
    }
}

/// Inspects tools and verifies host system prerequisites via `which`.
#[must_use]
pub(crate) fn validate_tools(tools: &[ToolDefinition]) -> ValidationReport {
    let mut report = ValidationReport {
        total_tools: tools.len(),
        ..Default::default()
    };

    for tool in tools {
        match tool.action {
            ToolAction::StaticCommand
            | ToolAction::ParameterizedCommand
            | ToolAction::CargoJsonDiagnostics => {
                if let Some(binary) = tool.primary_binary() {
                    if which::which(binary).is_ok() {
                        report.verified_tools = report.verified_tools.saturating_add(1);
                    } else {
                        report.missing_binaries.push((tool.name.clone(), binary.to_owned()));
                    }
                } else {
                    report.warnings.push(format!("Tool '{}' has command action but empty command vector", tool.name));
                }
            }
            ToolAction::FileRead
            | ToolAction::FileWrite
            | ToolAction::MpatchEdit
            | ToolAction::FileSearch
            | ToolAction::CliManual => {
                // In-process native actions are always verified
                report.verified_tools = report.verified_tools.saturating_add(1);
            }
            ToolAction::Unknown(ref act) => {
                report.warnings.push(format!("Tool '{}' specifies unrecognized action '{act}'", tool.name));
            }
        }
    }

    report
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_native_tools_verified() {
        let tools = vec![
            ToolDefinition {
                name: "read_file".to_owned(),
                action: ToolAction::FileRead,
                ..Default::default()
            },
            ToolDefinition {
                name: "run_nonexistent_binary_xyz".to_owned(),
                action: ToolAction::StaticCommand,
                command: vec!["nonexistent_binary_xyz_123".to_owned()],
                ..Default::default()
            },
        ];

        let report = validate_tools(&tools);
        assert_eq!(report.total_tools, 2);
        assert_eq!(report.verified_tools, 1);
        assert_eq!(report.missing_binaries.len(), 1);
        assert_eq!(report.missing_binaries[0].0, "run_nonexistent_binary_xyz");
    }
}
