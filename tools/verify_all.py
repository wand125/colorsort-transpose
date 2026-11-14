"""solutions/ の全手順を最初から再生して確かめ、bounds.tsv の上界と突き合わせる。

- best.tsv・docs_known.tsv・halving_13_24.tsv の各行を再生し、完成するか・手数が moves 列と合うかを見る。
- bounds.tsv の上界が、その N の確かめ済み手順の最小手数と一致するかを見る（N≤5 の確定値は除く）。
- `--readme` を付けると、README.ja.md（日本語）と README.md（英語）の <!-- bounds:start --> 〜 <!-- bounds:end --> の表を bounds.tsv から作り直す。
使い方: python3 tools/verify_all.py [--readme]   不一致があれば終了コード 1。
"""
import csv, os, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
D = "0123456789abcdefghijklmnopqrstuvwxyz"
FILES = [("solutions/best.tsv", "moves36"), ("solutions/docs_known.tsv", "moves36"), ("solutions/halving_13_24.tsv", "solution"), ("solutions/recur.tsv", "solution")]


def replay(n, s):
    L = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    if len(s) % 2:
        return False
    for i in range(0, len(s), 2):
        a, b = D.index(s[i]), D.index(s[i + 1])
        if a > n or b > n or a == b or not L[a] or len(L[b]) >= n:
            return False
        L[b].append(L[a].pop())
    return all(not l or (len(l) == n and len(set(l)) == 1) for l in L)


def main():
    bad = 0
    best = {}
    for fn, col in FILES:
        for r in csv.DictReader(open(os.path.join(ROOT, fn)), delimiter="\t"):
            n, m, s = int(r["N"]), int(r["moves"]), r[col].strip()
            ok = replay(n, s) and len(s) // 2 == m
            print(f"{fn:32s} N={n:2d} moves={m:5d} {'OK' if ok else 'NG'}")
            if not ok:
                bad += 1
            elif n not in best or m < best[n]:
                best[n] = m
    rows = list(csv.DictReader(open(os.path.join(ROOT, "bounds.tsv")), delimiter="\t"))
    for r in rows:
        n, up = int(r["N"]), int(r["upper"])
        if n <= 5:
            continue
        if best.get(n) != up:
            print(f"bounds.tsv N={n}: 上界 {up} と確かめ済み手順の最小 {best.get(n)} が合わない")
            bad += 1
    if "--readme" in sys.argv:
        a, b = "<!-- bounds:start -->", "<!-- bounds:end -->"
        ja = ["| N | 下界 | 下界の根拠 | 上界 | 上界の出所 | 予想 (2N−1)(N−1) |", "|---|---|---|---|---|---|"]
        en = ["| N | lower bound | upper bound | status | (2N−1)(N−1) |", "|---|---|---|---|---|"]
        for r in rows:
            n = int(r["N"])
            ja.append(f"| {n} | {r['lower']} | {r['lower_basis']} | {r['upper']} | {r['upper_source']} | {(2*n-1)*(n-1)} |")
            st = "**exact**" if r["lower"] == r["upper"] else "open"
            en.append(f"| {n} | {r['lower']} | {r['upper']} | {st} | {(2*n-1)*(n-1)} |")
        # 日本語の表は README.ja.md、英語の表は README.md
        for name, lines in (("README.ja.md", ja), ("README.md", en)):
            p = os.path.join(ROOT, name)
            txt = open(p).read()
            assert a in txt and b in txt, f"{name} に bounds の印が無い"
            txt = txt[: txt.index(a) + len(a)] + "\n" + "\n".join(lines) + "\n" + txt[txt.index(b):]
            open(p, "w").write(txt)
        print("README.ja.md と README.md の表を作り直した")
    print("不一致", bad)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
