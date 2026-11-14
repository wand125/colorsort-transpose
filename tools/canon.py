"""手順の正規形。同じ手数の手順が何通りもあるときに、代表を 1 つに決めるために使う。

次の 2 つの違いしかない手順は、同じ正規形になる。
1. レーン 1..N の番号の付け方（初期配置はどれも同じ中身なので、入れ替えても同じ手順）。
   → 完成したときに色が左から番号順に並ぶ番号に付け替える。空で終わるレーンは右端。レーン 0 は動かさない。
2. 使うレーンが 1 本も重ならない 2 手の順番（どちらを先にしても同じ）。
   → 入れ替えられる範囲で、36 進の文字列が辞書順で最小になる順番に並べる。
"""
import heapq

D = "0123456789abcdefghijklmnopqrstuvwxyz"


def replay(n, s):
    """最初から再生して完成するか。"""
    lanes = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    if len(s) % 2:
        return False
    for i in range(0, len(s), 2):
        a, b = D.find(s[i]), D.find(s[i + 1])
        if a < 0 or b < 0 or a > n or b > n or a == b or not lanes[a] or len(lanes[b]) >= n:
            return False
        lanes[b].append(lanes[a].pop())
    return all(not l or (len(l) == n and len(set(l)) == 1) for l in lanes)


def normalize(n, s):
    lanes = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    for i in range(0, len(s), 2):
        a, b = D.index(s[i]), D.index(s[i + 1])
        lanes[b].append(lanes[a].pop())
    order = sorted(range(1, n + 1), key=lambda i: lanes[i][0] if lanes[i] else n + 1)
    to = {0: 0}
    for new, old in enumerate(order, 1):
        to[old] = new
    moves = [(to[D.index(s[i])], to[D.index(s[i + 1])]) for i in range(0, len(s), 2)]
    # 手 j は、同じレーンを使う直前の手が済むまで打てない。打てる手のうち最小のものから取る。
    last, need, after = {}, [0] * len(moves), [[] for _ in moves]
    for j, (a, b) in enumerate(moves):
        for lane in (a, b):
            if lane in last and (not after[last[lane]] or after[last[lane]][-1] != j):
                after[last[lane]].append(j)
                need[j] += 1
            last[lane] = j
    heap = [(moves[j], j) for j in range(len(moves)) if need[j] == 0]
    heapq.heapify(heap)
    out = []
    while heap:
        mv, j = heapq.heappop(heap)
        out.append(D[mv[0]] + D[mv[1]])
        for k in after[j]:
            need[k] -= 1
            if need[k] == 0:
                heapq.heappush(heap, (moves[k], k))
    return "".join(out)
