//! Compiler span parsing and diagnostic compression.

use serde::{Deserialize, Serialize};

/// Compressed compiler diagnostic span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSpan {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub code: Option<String>,
    pub message: String,
    pub explanation: Option<String>,
}

/// Compact compiler error report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompactErrorReport {
    pub errors: Vec<DiagnosticSpan>,
    pub total_errors: usize,
}
