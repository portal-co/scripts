#!/bin/sh
ls | forfiles '^' sh -c "cd ^; git pull --no-rebase"
