# `CARGO_TARGET_DIR` in Cursor agent shells

## Problem

Cursor’s integrated agent terminal often exports `CARGO_TARGET_DIR` to a path under something like `…/cursor-sandbox-cache/…/cargo-target`. Cargo then writes build artifacts there instead of into the repository’s `./target/` directory.

This repo’s tooling assumes a normal workspace layout:

- [`crates/run`](../../crates/run) compares source mtimes against `<workspace>/target/debug/<binary>` and runs `cargo build` before `exec`’ing that path.
- [`bootstrap.sh`](../../bootstrap.sh) and [`tools/git/bootstrap.sh`](../../tools/git/bootstrap.sh) run `cargo build` and then point at `target/debug/run`.

When `CARGO_TARGET_DIR` points at a sandbox, `cargo build` succeeds but binaries land in the cache. Staleness checks, shims, and ad‑hoc `target/debug/...` invocations disagree with where Cargo actually wrote files. Sessions look “normal” until you hit test or exec phases.

## Mitigation (implemented here)

1. **`run` crate** — Before spawning nested `cargo build`, if we detect a Cursor agent / sandbox layout, we call `env_remove("CARGO_TARGET_DIR")` on that child process only. The parent shell keeps its environment; subsequent non-`run` commands are unchanged.

2. **Bootstrap scripts** — The same detection runs immediately before `cargo build -p run`, unsetting `CARGO_TARGET_DIR` in the current shell for that script so `target/debug/run` matches the build output.

Detection heuristics (intentionally redundant):

- `CURSOR_AGENT` or `CURSOR_TRACE_ID` set (typical Cursor agent sessions).
- Or `CARGO_TARGET_DIR` contains `cursor-sandbox-cache` (catches sandbox redirects even when agent-specific vars are missing).

## Recommended isolation instead

For reproducible builds and CI parity, prefer mechanisms that do not repoint Cargo’s target dir in surprising ways:

- **Git worktrees** — separate working trees share the same `.git` object store but can use distinct build directories naturally if you keep default `target/` per tree.
- **Explicit sandboxes** — containers, VMs, or documented `CARGO_TARGET_DIR` you control end-to-end (and teach tooling to match), rather than an IDE-injected path that diverges from on-disk paths hard-coded in scripts.

The unset logic here is a pragmatic fix so agents and humans do not have to special-case every `cargo` invocation in this repo; it is not a substitute for a clean, documented build environment when you need strict hermeticity.
