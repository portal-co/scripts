#!/bin/sh
ls | forfiles '^' sh -c 'cd ^; cargo update; git add -A; git commit -m update;'
