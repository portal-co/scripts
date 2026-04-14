#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
. "$(dirname $0)/bootstrap.sh"
ls | "$RUN" forfiles '^' sh -c 'cd ^; cargo update; git add -A; git commit -m update;'
