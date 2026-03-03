#!/bin/sh
ls | forfiles '^^' sh -c "cd ^^;forfiles '^' sh -c 'cd ^; git add -A; git commit -m update;'"