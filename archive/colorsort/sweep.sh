#!/bin/bash
# 幅・規則・P の重みを変えて 8 色と 12 色の最良手数を探す
cd ~/colorsort
run() { n=$1; w=$2; shift 2; tag="N$n w$w $*"; out=$(nice -n 10 ./target/release/colorsort $n $w "$@" 2>/dev/null); echo -e "$tag\t$(echo "$out" | head -1)\t$(echo "$out" | sed -n 2p)" >> results.tsv; }
for o in "--r2" "" "--r1 --r2"; do for l in 100 50 150; do run 8 1000000 $o --lambda $l; done; done
echo ALLDONE >> results.tsv
