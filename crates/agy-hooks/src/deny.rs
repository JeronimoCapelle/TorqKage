//! `PreToolUse` hard denial gate for deauthorized agents.

use agy_common::{PreToolResponse, PreToolUseInput};
use crate::stdin::read_stdin_payload;

/// Executes the `deny-all` hook.
///
/// Emits an immediate hard block verdict on `stdout` without spawning subprocesses.
pub(crate) fn execute() {
    let _input: PreToolUseInput = read_stdin_payload();

    let resp = PreToolResponse::deny(
        "Security policy: All tools are strictly deauthorized for this agent.".to_owned(),
    );

    if let Ok(json) = serde_json::to_string(&resp) {
        println!("{json}");
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_deny_all_output_format() {
        let resp = PreToolResponse::deny("Security policy: All tools are strictly deauthorized for this agent.".to_owned());
        let json = serde_json::to_string(&resp).expect("serialize failed");
        assert_eq!(
            json,
            r#"{"decision":"deny","reason":"Security policy: All tools are strictly deauthorized for this agent."}"#
        );
    }
}
