#!/bin/sh
# usage: bench.sh <sspur-bin> "<envA>" "<envB>"; best-of-N wall time per bench/native program, A and B interleaved
here=$(cd "$(dirname "$0")/../.." && pwd)
bin=$1
A=$2
B=$3
N=${N:-5}
now() { perl -MTime::HiRes=time -e 'printf "%.4f", time'; }
for b in ${BENCHES:-compute_big typical strings_big app churn parallel simd}; do
  f="$here/bench/native/$b.ssp"
  env $A ~/code/sspur-tools/tmo 300 "$bin" run "$f" > /dev/null 2>&1
  env $B ~/code/sspur-tools/tmo 300 "$bin" run "$f" > /dev/null 2>&1
  ba=999; bb=999; i=0
  while [ $i -lt $N ]; do
    s=$(now); env $A ~/code/sspur-tools/tmo 300 "$bin" run "$f" > /dev/null 2>&1; e=$(now)
    ba=$(echo "t = $e - $s; if (t < $ba) t else $ba" | bc)
    s=$(now); env $B ~/code/sspur-tools/tmo 300 "$bin" run "$f" > /dev/null 2>&1; e=$(now)
    bb=$(echo "t = $e - $s; if (t < $bb) t else $bb" | bc)
    i=$((i + 1))
  done
  echo "$b A=$ba B=$bb"
done
