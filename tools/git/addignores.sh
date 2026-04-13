#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
ls | run forfiles '^' sh -c 'cd ^;echo >> .gitignore; echo target >> .gitignore; echo node_modules >> .gitignore; echo .DS_Store >> .gitignore'
sh $(dirname $0)/sortallignores.sh
