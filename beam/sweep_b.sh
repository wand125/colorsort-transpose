#!/bin/bash
# 9〜11 色。幅 10 万で 3 通りの規則、そのあと幅 100 万で R2 と制限なし
cd ~/colorsort3
run() { n=$1; w=$2; shift 2; out=$(nice -n 10 ./target/release/colorsort $n $w "$@" 2>/dev/null); echo -e "N$n w$w $*\t$(echo "$out" | head -1)\t$(echo "$out" | sed -n 2p)" >> results_b.tsv; }
for n in 9 10 11; do for o in "--r2" "" "--r1 --r2"; do run $n 100000 $o; done; done
for n in 9 10 11; do for o in "--r2" ""; do run $n 1000000 $o; done; done
echo ALLDONE >> results_b.tsv
