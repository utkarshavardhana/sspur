#!/bin/sh
# usage: edit.sh <sspur-bin> <n-functions> <first-edit-id> [env...]; cold build, then 3 one-function edits, then a warm re-run
here=$(cd "$(dirname "$0")/../.." && pwd)
bin=$1; n=$2; e=$3; shift 3
mkdir -p "$here/target/inc"
LC_ALL=en_US.UTF-8 python3 "$here/bench/incremental/gen.py" "$n" "$e" > "$here/target/inc/e0.ssp"
printf "cold  "; env "$@" sh "$here/bench/incremental/run.sh" "$bin" "$here/target/inc/e0.ssp"
for k in 1 2 3; do
  LC_ALL=en_US.UTF-8 python3 "$here/bench/incremental/gen.py" "$n" $((e + k)) > "$here/target/inc/e$k.ssp"
  printf "edit  "; env "$@" sh "$here/bench/incremental/run.sh" "$bin" "$here/target/inc/e$k.ssp"
done
printf "warm  "; env "$@" sh "$here/bench/incremental/run.sh" "$bin" "$here/target/inc/e3.ssp"
