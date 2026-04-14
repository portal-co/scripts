#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
. "$(dirname $0)/bootstrap.sh"
ls | "$RUN" forfiles '^' sh -c "sh $(dirname $0)/../splat/splat.sh '^'"
