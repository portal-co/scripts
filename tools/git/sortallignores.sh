#!/bin/sh
ls | forfiles '^' sh -c 'cd ^; (cat .gitignore || true) | sort | uniq > .gitignore.2; mv .gitignore.2 .gitignore'
