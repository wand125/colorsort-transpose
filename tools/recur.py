"""案 3-b: 半分割りの上半分のあと、回文レーン [1..m, m..1] の上の層を m 色の元の問題として解く序盤を作る。

上半分を終えた盤面は「回文レーン [1..k1, k2..1] が k2 本 + 空レーン 1 本 + 完成した色レーンなど」になる（k1 = ⌈N/2⌉、k2 = ⌊N/2⌋、m = k2）。
奇数 N では下の層が [1..k1] と 1 つ長く、注ぎ切れなかったレーン [1..k2, k1×k1] が 1 本残るが、上の層の形は同じ。
回文レーンの上の層 [m..1] は、色を c → m+1−c と付け替えると m 色の初期配置 [1..m] と同じ形で、下の層が埋まっているので
容量 m の盤として動く。空レーンは容量 N だが、m 色の手順は容量 m を守るのでそのまま使える。
m 色の最良解（solutions/ の再生検証済みの手順）をレーン番号だけ付け替えて再生し、そこまでを序盤として出す。
使い方: recur.py N   出力: 1 行目に説明、2 行目に序盤の手順（36 進 2 文字で 1 手）。残りは fromopen.py でビームに解かせる。
"""
import csv, os, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
D = "0123456789abcdefghijklmnopqrstuvwxyz"


def best_solution(m):
    best = None
    for fn, col in (("solutions/best.tsv", "moves36"), ("solutions/docs_known.tsv", "moves36")):
        for r in csv.DictReader(open(os.path.join(ROOT, fn)), delimiter="\t"):
            if int(r["N"]) == m and (best is None or int(r["moves"]) < best[0]):
                best = (int(r["moves"]), r[col].strip())
    return best


def main():
    n = int(sys.argv[1])
    k1, k2 = (n + 1) // 2, n // 2
    m = k2  # 上の層の長さ（奇数 N では下の層が 1 つ長い [1..k1]）
    pre = subprocess.run([sys.executable, os.path.join(ROOT, "tools/halving.py"), str(n), "--prefix"], capture_output=True, text=True).stdout.split("\n")[1].strip()
    L = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    for i in range(0, len(pre), 2):
        a, b = D.index(pre[i]), D.index(pre[i + 1])
        L[b].append(L[a].pop())
    pal = list(range(1, k1 + 1)) + list(range(k2, 0, -1))
    lanes = [i for i, l in enumerate(L) if l == pal]
    empty = [i for i, l in enumerate(L) if not l]
    assert len(lanes) == m and len(empty) == 1, (len(lanes), len(empty))
    sol = best_solution(m)
    assert sol, f"{m} 色の解が無い"
    mp = [empty[0]] + lanes  # m 色の盤のレーン 0（空）→ 実際の空レーン、1..m → 回文レーン
    moves = pre
    for i in range(0, len(sol[1]), 2):
        a, b = D.index(sol[1][i]), D.index(sol[1][i + 1])
        ra, rb = mp[a], mp[b]
        assert L[ra] and len(L[rb]) < n
        L[rb].append(L[ra].pop())
        moves += D[ra] + D[rb]
    print(f"N={n} 上半分 {len(pre)//2} 手 + {m} 色の解 {sol[0]} 手 = 序盤 {len(moves)//2} 手")
    print(moves)


if __name__ == "__main__":
    main()
