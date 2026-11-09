//! Stdin JSON deserialization helper for Antigravity hooks.

use std::io::IsTerminal;

/// Reads and deserializes a JSON payload from `stdin`.
///
/// If `stdin` is connected to a terminal (interactive human execution) or empty,
/// returns `T::default()` instead of blocking waiting for EOF.
#[must_use]
pub(crate) fn read_stdin_payload<T: serde::de::DeserializeOwned + Default>() -> T {
    if std::io::stdin().is_terminal() {
        return T::default();
    }

    let mut buffer = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer).is_err() {
        return T::default();
    }

    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return T::default();
    }

    match serde_json::from_str::<T>(trimmed) {
        Ok(val) => val,
        Err(err) => {
            eprintln!("tkg-hooks: warning: failed to parse stdin JSON payload: {err}");
            T::default()
        },
    }
}
