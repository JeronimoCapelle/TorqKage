# Bubblewrap Sandboxing & Jails

Security in `agy-suite` operates on a **default-deny capability model**.

## Container Architecture (`bwrap`)

Subprocesses spawned by agents are wrapped in ephemeral [Bubblewrap](https://github.com/containers/bubblewrap) containers via `agy_common::sandbox::BubblewrapBuilder`:

```
┌────────────────────────────────────────────────────────┐
│             Bubblewrap Isolation Container             │
│                                                        │
│  [Read-Only System Roots]                              │
│  /usr, /bin, /lib, /lib64, /etc (ro)                   │
│                                                        │
│  [Private Namespaces]                                  │
│  --unshare-all (IPC, PID, Network, UTS, Cgroup)        │
│  --die-with-parent (Orphan prevention)                 │
│  --new-session (Process group isolation)               │
│                                                        │
│  [Network Air-Gapping]                                 │
│  Private loopback (lo), external interfaces unmounted  │
│                                                        │
│  [Credential Masking]                                  │
│  .env, .git/credentials, ~/.ssh mounted to /dev/null   │
│                                                        │
│  [Workspace Confinement]                               │
│  Workspace root mounted read-write                     │
└────────────────────────────────────────────────────────┘
```

## Nested Container Detection

To prevent container-in-container crashes inside Docker, Podman, or CI environments, `BubblewrapBuilder` automatically detects existing container roots via `/.dockerenv` or `/run/.containerenv`. When running inside an already containerized environment, it gracefully degrades to direct execution while maintaining path confinement.
