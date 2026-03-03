#!/bin/sh
ls | forfiles '^' sh -c 'cd ^; cargo fmt; git add -A; git commit -m fmt;'
