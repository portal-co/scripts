#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
. "$(dirname $0)/bootstrap.sh"
ls | "$RUN" forfiles -C '^' '^' sh -c '(cat .gitignore || true) | sort | uniq > .gitignore.2; mv .gitignore.2 .gitignore'
