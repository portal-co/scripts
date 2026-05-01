#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
. "$(dirname $0)/bootstrap.sh"
ls | "$RUN" forfiles '^^' sh -c "cd ^^;$RUN retry-forfiles '^' sh -c 'cd ^; git pull --no-rebase'"
