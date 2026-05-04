#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
. "$(dirname $0)/bootstrap.sh"
ls -d */*/ | "$RUN" retry-forfiles -C '^' '^' git pull --no-rebase
