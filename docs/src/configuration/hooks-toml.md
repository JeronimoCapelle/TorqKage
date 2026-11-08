# Hooks Configuration (`hooks.toml`)

`hooks.toml` defines declarative lifecycle triggers executed by `agy-hooks`.

## Schema Example

```toml
[hooks.turn_one]
enabled = true
inject_vcs_status = true
inject_uncommitted_diff = true

[hooks.circuit_breaker]
enabled = true
max_consecutive_failures = 2

[hooks.path_policies]
enforce_tests_only = true
enforce_comments_only = true
```

These rules allow developers to customize how the agent fleet interacts with their codebase during interactive sessions.
