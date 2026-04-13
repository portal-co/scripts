#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
ls | run retry-forfiles '^' sh -c 'cd ^; git add -A; git commit -m update; git push'
