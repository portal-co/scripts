---
name: cleanse
description: >
  Close all open file descriptors above stderr (fd > 2) then exec a command,
  replacing the current process. Use when spawning a subprocess that should not
  inherit open fds from the parent — prevents fd leaks across exec boundaries,
  especially important in long-running daemon or pipeline contexts.
  Trigger on: "cleanse", "close file descriptors", "fd leak", "exec clean", "fd sanitize".
---

# cleanse

Closes every open file descriptor above `stderr` (fd > 2), then `exec`s the
given command, replacing the current process. Prevents fd leaks across exec
boundaries.

## Installation

```bash
cargo build -p cleanse
# or: cargo install --path crates/cleanse
./cleanse <command> [args...]
```

## Usage

### Run a command with a clean fd table

```bash
./cleanse ./my-script.sh --arg value
```

### Wrap another tool invocation

```bash
./cleanse ./forfiles {} cargo build --manifest-path {}/Cargo.toml
```

### Use in a pipeline where a parent holds many fds open

```bash
# Without cleanse, child inherits all parent fds (sockets, log files, etc.)
./cleanse ./deploy.sh

# The deploy script runs with only stdin/stdout/stderr open
```

## Behavior

- Iterates fds from 3 to `(1 << 24) - 1` and calls `close(2)` on each
- After closing, `exec`s the command — the `cleanse` process image is replaced
- On non-Unix platforms, falls back to `spawn + wait` (no fd table replacement)
- Any error from `exec` is a panic (the process would be in an indeterminate state)

## When to use

- Before running user-supplied commands in a pipeline orchestrator
- When spawning long-lived subprocesses from a daemon that holds many fds
- As a precaution before running scripts that themselves spawn children, to
  prevent accidental fd inheritance across multiple generations

## Notes

Closing up to `1 << 24` (≈16M) fds is fast on Linux/macOS because `close` on
an already-closed fd is a no-op. The cost is bounded by the kernel's fd table
size, not the loop count.
