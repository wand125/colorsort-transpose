#!/bin/bash
# 省メモリ版で 8 色と 12 色を回す。結果は results.tsv（設定、手数、手順）
cd ~/colorsort3
run() { n=$1; w=$2; shift 2; out=$(nice -n 10 ./target/release/colorsort $n $w "$@" 2>/dev/null); echo -e "N$n w$w $*\t$(echo "$out" | head -1)\t$(echo "$out" | sed -n 2p)" >> results.tsv; }
for o in "--r2" "" "--r1 --r2"; do run 8 100000 $o; done
for o in "--r2" "" "--r1 --r2"; do run 12 100000 $o; done
for o in "--r2" "" "--r1 --r2"; do for l in 100 50 150; do run 8 1000000 $o --lambda $l; done; done
for o in "--r2" ""; do run 12 1000000 $o; done
echo ALLDONE >> results.tsv
