"""2 本ずつ上下に分けてから、両方の山の上の層に m 色の最良解を当てはめる序盤を作る（偶数 N、m = N/2）。

1. レーンを (1,2), (3,4), … の組にし、空レーン E に A の上半分、B の上半分を移し、B の残りを A に移す（1 組 3m 手）。
   終わると、上半分の色の山 [N..m+1, N..m+1] が m 本（レーン 0, 2, …, N−2）、
   下半分の色の回文 [1..m, m..1] が m 本（レーン 1, 3, …, N−1）、空レーン N になる。全体で 0.75N² 手。
2. 回文の上の層 [m..1] は色を付け替えると m 色の初期配置と同じ形。空レーン N を使って m 色の最良解を再生する。
3. そのとき空いたレーン z（中身は下の層だけで、上に m 個の空きがある）を空レーン代わりに、
   山の上の層 [N..m+1] にも m 色の最良解を再生する。
使い方: split.py N [order]   order=lu（下→上、既定）か ul（上→下）。出力: 1 行目に説明、2 行目に序盤の手順。
残りは fromopen.py でビームに解かせる。
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from recur import best_solution, D


def main():
    n = int(sys.argv[1])
    order = sys.argv[2] if len(sys.argv) > 2 else "lu"
    assert n % 2 == 0
    m = n // 2
    L = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    moves = []

    def mv(a, b):
        assert L[a] and len(L[b]) < n, (a, b)
        L[b].append(L[a].pop())
        moves.append((a, b))

    e = 0
    for p in range(m):
        a, b = 2 * p + 1, 2 * p + 2
        for _ in range(m): mv(a, e)
        for _ in range(m): mv(b, e)
        for _ in range(m): mv(b, a)
        e = b
    assert e == n and not L[n]
    split_len = len(moves)
    sol = best_solution(m)
    seq = [(D.index(sol[1][i]), D.index(sol[1][i + 1])) for i in range(0, len(sol[1]), 2)]
    low = [2 * p + 1 for p in range(m)]
    up = [2 * p for p in range(m)]
    first, second = (low, up) if order == "lu" else (up, low)
    before = {i: len(L[i]) for i in range(n + 1)}
    mp = [n] + first
    for a, b in seq: mv(mp[a], mp[b])
    z = [r for r in first if len(L[r]) == before[r] - m] + ([n] if not L[n] else [])
    assert len(z) == 1, z
    mp = [z[0]] + second
    for a, b in seq: mv(mp[a], mp[b])
    s = "".join(D[a] + D[b] for a, b in moves)
    print(f"N={n} 分ける {split_len} 手 + {m} 色の解 {sol[0]} 手 ×2 = 序盤 {len(moves)} 手（{order}）")
    print(s)


if __name__ == "__main__":
    main()
