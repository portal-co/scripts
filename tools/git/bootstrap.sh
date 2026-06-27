#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
# Source this file to build the `run` auto-recompile runner and set $RUN.
#
#   . "$(dirname $0)/bootstrap.sh"
#
# After sourcing, $RUN holds the absolute path to target/debug/run and is
# ready to invoke other workspace binaries (forfiles, retry-forfiles, …).
_SCRIPTS_DIR="$(cd "$(dirname "$0")/../.." && pwd)"

# Cursor agent shells often set CARGO_TARGET_DIR to a sandbox cache; workspace
# scripts expect ./target/debug/run. See docs/design/cursor-cargo-target-dir.md.
if [ -n "${CURSOR_AGENT:-}" ] || [ -n "${CURSOR_TRACE_ID:-}" ]; then
  unset CARGO_TARGET_DIR
else
  case "${CARGO_TARGET_DIR:-}" in
    *cursor-sandbox-cache*) unset CARGO_TARGET_DIR ;;
  esac
fi

cargo build -p run --manifest-path "$_SCRIPTS_DIR/Cargo.toml" -q
RUN="$_SCRIPTS_DIR/target/debug/run"
