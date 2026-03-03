#!/bin/sh
ls | forfiles '^' sh -c "sh $(dirname $0)/../splat/splat.sh '^'"
