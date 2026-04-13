#!/bin/sh
# AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
ls | run forfiles '^' sh -c "sh $(dirname $0)/../splat/splat.sh '^'"
