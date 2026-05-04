#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
. "$(dirname $0)/bootstrap.sh"
ls | "$RUN" forfiles -C '^' '^' sh -c 'echo >> .gitignore; echo target >> .gitignore; echo node_modules >> .gitignore; echo .DS_Store >> .gitignore'
sh $(dirname $0)/sortallignores.sh
