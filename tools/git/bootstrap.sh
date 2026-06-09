#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
# Source this file to build the `run` auto-recompile runner and set $RUN.
#
#   . "$(dirname $0)/bootstrap.sh"
#
# After sourcing, $RUN holds the absolute path to target/debug/run and is
# ready to invoke other workspace binaries (forfiles, retry-forfiles, …).
_SCRIPTS_DIR="$(cd "$(dirname "$0")/../.." && pwd)"

cargo build -p run --manifest-path "$_SCRIPTS_DIR/Cargo.toml" -q
RUN="${CARGO_TARGET_DIR:-$_SCRIPTS_DIR/target}/debug/run"
