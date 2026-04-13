#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
ls | run forfiles '^^' sh -c "cd ^^;run forfiles '^' sh -c 'cd ^; git add -A; git commit -m update;'"