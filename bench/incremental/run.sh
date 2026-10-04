#!/bin/sh
# usage: run.sh <sspur-bin> <file.ssp> [args...]; prints wall seconds of `sspur test` (cache in target/inc/cache)
here=$(cd "$(dirname "$0")/../.." && pwd)
bin=$1; shift
f=$1; shift
export SSPUR_CACHE="${SSPUR_CACHE:-$here/target/inc/cache}"
s=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
~/code/sspur-tools/tmo 300 "$bin" test "$f" "$@" > "$here/target/inc/last.out" 2>&1
rc=$?
e=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
echo "rc=$rc $(tail -1 "$here/target/inc/last.out") time=$(echo "$e - $s" | bc)"
