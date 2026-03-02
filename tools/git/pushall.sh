#!/bin/sh
ls | forfiles '^' sh -c "cd ^; git push"
