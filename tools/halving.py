"""半分割りの組み立て（貪欲法）。12 色 272 手解の「2 本を注ぎ合わせて空レーンを作る」を一般の N に広げる。

段取り（N 色、k1 = ceil(N/2), k2 = floor(N/2)）
1. 一番上の色 N をレーン 0 に集める。
2. 上半分の色 c = N−1 … k1+1 を順に完成させる。空レーンが無ければ、底が [1..k] のまま残っている
   レーンから 2 本選んで上を崩し、[1..k2] を [1..k1] へ注いで空きを作る。注いだレーンは
   「下半分の置き場」として以後さわらない。
3. 残りの色（下半分）は、上にある色から順に「空きに集める」を繰り返す。

玉をどかす先は、同じ色の上 > 大きい色の上 > 空きの多いレーン の順に選ぶ。
使い方: halving.py N [--show] [--prefix]（--prefix は上半分と最後の注ぎ合わせまでの手順だけを出す）   出力: 手数と 36 進の手順（最初から再生して検証済み）
"""
import os, sys

VAR = os.environ.get("HV", "")  # 改善案の切り替え（B: 注ぐ候補を避ける、C: 掘るレーンの選び方）

D = "0123456789abcdefghijklmnopqrstuvwxyz"


class Stuck(Exception):
    pass


def solve(n, verbose=False, prefix_only=False):
    L = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    moves = []
    frozen = set()      # 完成したレーンと、注いだ下半分の置き場
    k1, k2 = (n + 1) // 2, n // 2

    def mv(a, b):
        assert L[a] and len(L[b]) < n and a != b, (a, b)
        L[b].append(L[a].pop())
        moves.append((a, b))

    def uniform_full(l):
        return len(l) == n and len(set(l)) == 1

    def dest_for(v, avoid):
        """玉 v の置き場。avoid は使わないレーン。"""
        best = None
        for b in range(n + 1):
            if b in avoid or b in frozen or len(L[b]) >= n or not L[b]:
                continue
            t = L[b][-1]
            pen = 0
            if "B" in VAR and len(L[b]) >= k2 and L[b][:k2] == list(range(1, k2 + 1)):
                pen = 1  # 注ぎ合わせの候補は汚さない
            # 同じ色の上 > 大きい色の上（大きいほど良い）> それ以外。空きが多い方を少し優先
            if t == v:
                key = (0, pen, 0, -(n - len(L[b])))
            elif t > v:
                key = (1 + pen, 0, -t, -(n - len(L[b])))
            else:
                key = (3 + pen, 0, -t, -(n - len(L[b])))
            if best is None or key < best[0]:
                best = (key, b)
        if best is None:
            raise Stuck(f"置き場なし v={v}")
        return best[1]

    def empty_lane():
        for i in range(n + 1):
            if not L[i] and i not in frozen:
                return i
        return None

    def collect(c, E, reserve=()):
        """色 c を空レーン E に n 個集める。"""
        while L[E].count(c) < n:
            # 上に c があるレーンから先に
            progressed = False
            for a in range(n + 1):
                if a != E and a not in frozen and L[a] and L[a][-1] == c:
                    mv(a, E)
                    progressed = True
            if progressed:
                continue
            # c が一番浅いレーンを掘る
            cand = []
            for a in range(n + 1):
                if a == E or a in frozen or c not in L[a]:
                    continue
                pos = max(i for i, x in enumerate(L[a]) if x == c)
                depth = len(L[a]) - 1 - pos
                cost = depth
                if "C" in VAR:
                    # 上の玉に同じ色の置き場が無ければ 1 手余計に数える
                    tops = [l[-1] for j, l in enumerate(L) if l and j not in frozen and j != a and len(l) < n]
                    cost = depth + sum(1 for v in L[a][pos + 1:] if v not in tops)
                cand.append((cost, depth, a))
            if not cand:
                raise Stuck(f"色 {c} が見つからない")
            a = min(cand)[-1]
            while L[a][-1] != c:
                v = L[a][-1]
                mv(a, dest_for(v, {a, E} | set(reserve)))
            mv(a, E)
        frozen.add(E)

    def pour_step():
        """底が [1..k] のレーン 2 本を崩して注ぎ、空レーンを作る。"""
        def excess(l, k):
            if len(l) < k or l[:k] != list(range(1, k + 1)):
                return None
            return len(l) - k
        cands = []
        for a in range(n + 1):
            if a in frozen:
                continue
            e = excess(L[a], k2)
            if e is not None:
                cands.append((e, a))
        if len(cands) < 2:
            raise Stuck("注ぐレーンが足りない")
        cands.sort()
        # 片方は [1..k1] まで、もう片方は [1..k2] まで崩す
        (e_a, a), (e_b, b) = cands[0], cands[1]
        A, B = (a, b) if excess(L[a], k1) is not None and (excess(L[a], k1) <= excess(L[b], k1) if excess(L[b], k1) is not None else True) else (b, a)
        for lane, k in ((B, k2), (A, k1)):
            while len(L[lane]) > k:
                v = L[lane][-1]
                mv(lane, dest_for(v, {A, B}))
        for _ in range(k2):
            mv(B, A)
        frozen.add(A)

    # 1. 色 N をレーン 0 へ
    for i in range(1, n + 1):
        mv(i, 0)
    frozen.add(0)
    # 2. 上半分
    for c in range(n - 1, k1, -1):
        E = empty_lane()
        if E is None:
            pour_step()
            E = empty_lane()
        collect(c, E)
        if verbose:
            print(f"色 {c} 完成 {len(moves)} 手")
    # 上半分の最後に残った [1..k] の 2 本を注いで、下半分用の空レーンを作る
    if empty_lane() is None:
        pour_step()
    if prefix_only:
        return moves
    # 3. 下半分: 注いだレーンも含めて、上に多く出ている色から集める
    frozen = {i for i in range(n + 1) if uniform_full(L[i])}
    remaining = [c for c in range(1, k1 + 1) if not any(uniform_full(L[i]) and L[i][0] == c for i in range(n + 1))]
    while remaining:
        E = empty_lane()
        if E is None:
            raise Stuck("下半分で空レーンが無い")
        # 一番上に出ている数が多い色を選ぶ
        def tops(c):
            return sum(1 for l in L if l and l[-1] == c)
        c = max(remaining, key=lambda c: (tops(c), -c))
        collect(c, E)
        remaining.remove(c)
        if verbose:
            print(f"色 {c} 完成 {len(moves)} 手")
    return moves


def verify(n, moves):
    L = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    for a, b in moves:
        if not L[a] or len(L[b]) >= n:
            return False
        L[b].append(L[a].pop())
    return all(not l or (len(l) == n and len(set(l)) == 1) for l in L)


if __name__ == "__main__":
    n = int(sys.argv[1])
    try:
        mv = solve(n, "--show" in sys.argv, "--prefix" in sys.argv)
    except Stuck as e:
        print(f"N={n} 行き詰まり: {e}")
        sys.exit(1)
    if "--prefix" in sys.argv:
        print(f"N={n} prefix={len(mv)}")
    else:
        print(f"N={n} moves={len(mv)} verified={verify(n, mv)} formula={(2*n-1)*(n-1)}")
    print("".join(D[a] + D[b] for a, b in mv))
