#!/bin/bash
# 6 色の下界を B=51..55 で順に証明する。空きメモリが 6 GB を切ったら止める（常駐プロセスを守る）
cd ~/colorlb
for b in 51 52 53 54 55; do
  ./target/release/colorlb prove 6 $b > prove6_$b.log 2>&1 &
  pid=$!
  while kill -0 $pid 2>/dev/null; do
    avail=$(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo)
    if [ "$avail" -lt 6 ]; then kill $pid; echo "STOPPED: MemAvailable ${avail}GB" >> prove6_$b.log; exit 1; fi
    sleep 5
  done
  grep -q "T ≥" prove6_$b.log || break
done
