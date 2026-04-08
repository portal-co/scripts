#!/usr/bin/env bash
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
# bootstrap.sh — Build the `run` auto-recompile runner.
#
# Run this once after cloning, or whenever you update crates/run/src/main.rs.
# After bootstrap, the wrapper scripts (inject-key, bump-npm-version, …) will
# automatically rebuild their target binaries on first use.
#
# Usage:
#   ./bootstrap.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "bootstrap: building run …"
cargo build -p run --manifest-path "$SCRIPT_DIR/Cargo.toml"
echo "bootstrap: done — target/debug/run is ready"
echo ""
echo "The following wrappers will now auto-rebuild their binaries on source changes:"
for f in "$SCRIPT_DIR"/inject-key \
          "$SCRIPT_DIR"/bump-npm-version \
          "$SCRIPT_DIR"/agents-zip \
          "$SCRIPT_DIR"/scaffold-repo \
          "$SCRIPT_DIR"/copy-feed-files \
          "$SCRIPT_DIR"/forfiles \
          "$SCRIPT_DIR"/cleanse; do
  if [[ -x "$f" ]]; then
    echo "  $(basename "$f")"
  fi
done
