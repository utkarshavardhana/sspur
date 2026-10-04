#!/bin/sh
# usage: rss.sh <sspur-bin> <file.ssp> [env...]; prints wall time and peak resident memory of one `sspur run`
bin=$1; f=$2; shift 2
env "$@" /usr/bin/time -l ~/code/sspur-tools/tmo 300 "$bin" run "$f" 2>&1 >/dev/null | awk '/real/ {t=$1} /maximum resident/ {m=$1} END {printf "%.3fs %.0f MB\n", t, m / 1048576}'
