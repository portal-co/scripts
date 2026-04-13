#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
ls | run forfiles '^' sh -c 'cd ^; cargo fmt; git add -A; git commit -m fmt;'
