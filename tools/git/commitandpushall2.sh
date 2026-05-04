#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
. "$(dirname $0)/bootstrap.sh"
ls -d */*/ | "$RUN" retry-forfiles -C '^' '^' sh -c 'git add -A; git commit -m update; git push'
