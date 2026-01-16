#!/bin/sh
ls | go run $(dirname $0)/../forfiles/forfiles.go '^' sh -c "sh $(dirname $0)/../splat/splat.sh '^'"
