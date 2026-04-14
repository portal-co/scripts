#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
# Generate ../.cargo/config.toml patch sections from the repos in a directory.
#
# Usage:
#   gen-patch.sh [REPOS_DIR] [--output <FILE>]
#
# REPOS_DIR defaults to the parent of this script's scripts repo (i.e. the
# directory that contains the individual repo checkouts alongside the scripts
# repo itself).  Pass a different path to point at any collection of repos.
. "$(dirname $0)/bootstrap.sh"
"$RUN" gen-patch "$@"
