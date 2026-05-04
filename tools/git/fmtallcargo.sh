#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
. "$(dirname $0)/bootstrap.sh"
ls | "$RUN" forfiles -C '^' '^' sh -c 'cargo fmt; git add -A; git commit -m fmt'
