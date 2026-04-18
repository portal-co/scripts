---
name: run
description: >
  Auto-recompile and exec a workspace binary when its source is newer than the
  built artifact, then replace the current process with it. Use when invoking
  Rust script binaries during development without manually running cargo build,
  or when writing shell shims that should auto-rebuild on source changes.
  Trigger on: "run binary", "auto rebuild", "run crate", "dev runner", "script shim".
---

# run

Auto-recompiles a workspace binary when any source file (`.rs` or `Cargo.toml`)
inside its crate is newer than the built artifact, then `exec`s the binary —
replacing the current process. If the binary is already up to date, exec is
immediate (no build overhead).

## Installation

```bash
cargo build -p run
# Binary lives at: target/debug/run
# (run bootstraps itself — build once, then it self-maintains)
```

## Usage

```bash
./target/debug/run <binary-name> [args...]
```

`<binary-name>` must correspond to a crate at `crates/<binary-name>/` in the
workspace.

### Run any workspace tool with auto-rebuild

```bash
./target/debug/run freeze record deploy.sh
./target/debug/run inject-key --rotate
./target/debug/run gen-patch ~/Code/portal-hot
```

### Shell shim pattern (how all scripts in this repo work)

Each tool has a thin shell wrapper that delegates to `run`:

```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/target/debug/run" "$(basename "$0")" "$@"
```

This means `./freeze`, `./inject-key`, etc. all auto-rebuild their Rust binary
if the source has changed since the last build.

## How it decides to rebuild

1. Lists all `.rs` files and `Cargo.toml` inside `crates/<binary>/` recursively
2. Finds the newest mtime among them
3. Compares to the mtime of `target/debug/<binary>`
4. If any source is newer (or binary is absent), runs `cargo build -p <binary>`
5. `exec`s the binary, replacing the current process

## Notes on workspace root

The workspace root is baked in at compile time via `env!("CARGO_MANIFEST_DIR")`,
so `run` works correctly when invoked from any directory — not just the workspace
root.

## Creating a new shim

When adding a new crate `crates/my-tool/`:

```bash
# Create the shell shim
cat > my-tool <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/target/debug/run" "$(basename "$0")" "$@"
EOF
chmod +x my-tool
```

Then add `"crates/my-tool"` to the workspace `members` in `Cargo.toml`.
