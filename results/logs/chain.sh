#!/bin/bash
# B=51 の終了を待ち、52〜55 を順に証明する。解に届いたら止める
cd ~/colorlb
until grep -q -E "T ≥|T ≤" prove6_51.log 2>/dev/null; do sleep 30; done
grep -q "T ≤" prove6_51.log && exit 0
for b in 52 53 54 55; do
  ./target/release/colorlb prove 6 $b > prove6_$b.log 2>&1
  grep -q "T ≤" prove6_$b.log && break
  grep -q "T ≥" prove6_$b.log || break
done
