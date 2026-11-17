"""公開サイト（docs/）のデータと表を、bounds.tsv と solutions/ から作り直す。

- docs/data/bounds.json: 色数ごとの下界・上界と根拠（英語と日本語）。
- docs/data/solutions.json: 色数ごとの最良手順（再生して確かめ、tools/canon.py の正規形にしたもの）。
- docs/*.html の <!-- bounds:start --> 〜 <!-- bounds:end --> を表の行で置き換える。
使い方: python3 tools/build_site.py
"""
import csv, json, os, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
from canon import D, normalize, replay  # noqa: E402


def general(n):
    return (3 * n * n - n - 4) // 2


def lower_kind(n, lower, upper):
    if n <= 5:
        return ("exhaustive search", "全探索")
    if lower > general(n):
        return ("pruned search with the bound J", "下界 J を使った枝刈り全探索")
    return ("general bound (3N²−N−4)/2", "一般式 (3N²−N−4)/2")


def upper_kind(n, text):
    # External submissions form the "submission" source kind.
    if text.startswith("外部の提出: @"):
        login = text.removeprefix("外部の提出: @")
        if login and len(login) <= 39 and all(c in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-" for c in login):
            return (f"submitted by @{login}", f"外部の提出: @{login}")
    if n <= 5:
        return ("optimal solution", "最短の手順")
    if "r1r2" in text:
        return ("search restricted by two rules", "2 つの規則で制限した探索")
    if "既知" in text:
        return ("earlier recorded solution", "以前から記録されていた手順")
    if "半分割り" in text:
        return ("construction and beam search", "組み立てとビーム探索")
    return ("beam search", "ビーム探索")


def bfs(n):
    start = tuple([()] + [tuple(range(1, n + 1))] * n)
    seen, layer = {start: ""}, [start]
    while layer:
        nxt = []
        for s in layer:
            if all(not l or (len(l) == n and len(set(l)) == 1) for l in s):
                return seen[s]
            for a in range(n + 1):
                if not s[a]:
                    continue
                for b in range(n + 1):
                    if a == b or len(s[b]) >= n:
                        continue
                    t = list(s); t[b] = s[b] + (s[a][-1],); t[a] = s[a][:-1]; t = tuple(t)
                    if t not in seen:
                        seen[t] = seen[s] + D[a] + D[b]; nxt.append(t)
        layer = nxt


def main():
    rows = list(csv.DictReader(open(ROOT / "bounds.tsv", encoding="utf-8"), delimiter="\t"))
    best = {}
    for path in sorted((ROOT / "solutions").glob("*.tsv")):
        for r in csv.DictReader(open(path, encoding="utf-8-sig"), delimiter="\t"):
            s = (r.get("moves36") or r.get("solution") or "").strip()
            n = int(r["N"])
            if not replay(n, s):
                continue
            t = normalize(n, s)
            assert replay(n, t) and len(t) == len(s), (path.name, n)
            m = len(s) // 2
            if n not in best or (m, t) < (best[n]["moves"], best[n]["s"]):
                best[n] = dict(moves=m, s=t, file="solutions/" + path.name)
    for n in (2, 3):
        s = normalize(n, bfs(n))
        best[n] = dict(moves=len(s) // 2, s=s, file=None)
    bounds, bad = [], 0
    for r in rows:
        n, lo, up = int(r["N"]), int(r["lower"]), int(r["upper"])
        if n not in best or best[n]["moves"] != up:
            print(f"N={n}: 上界 {up} と手順の手数 {best.get(n, {}).get('moves')} が合わない"); bad += 1
        lk, uk = lower_kind(n, lo, up), upper_kind(n, r["upper_source"])
        bounds.append(dict(N=n, lower=lo, upper=up, exact=lo == up, general=general(n), fit=(2 * n - 1) * (n - 1),
                           lower_en=lk[0], lower_ja=lk[1], upper_en=uk[0], upper_ja=uk[1]))
        best[n].update(lower=lo, exact=lo == up, kind_en=uk[0], kind_ja=uk[1])
    out = ROOT / "docs" / "data"
    out.mkdir(parents=True, exist_ok=True)
    json.dump(bounds, open(out / "bounds.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    json.dump({str(n): v for n, v in sorted(best.items())}, open(out / "solutions.json", "w", encoding="utf-8"), ensure_ascii=False)

    def table(lang, full):
        tag = lambda b: '<span class="tag exact">%s</span>' % ("exact" if lang == "en" else "確定") if b["exact"] else '<span class="tag">%s</span>' % ("open" if lang == "en" else "未確定")
        lines = []
        for b in bounds:
            cells = [f'<td class="num">{b["N"]}</td>', f'<td class="num">{b["lower"]:,}</td>', f'<td class="num">{b["upper"]:,}</td>', f'<td>{tag(b)}</td>']
            if full:
                cells += [f'<td>{b["lower_" + lang]}</td>', f'<td>{b["upper_" + lang]}</td>']
            cells.append(f'<td class="num">{b["fit"]:,}</td>')
            lines.append("    <tr>" + "".join(cells) + "</tr>")
        return "\n".join(lines)

    a, z = "<!-- bounds:start -->", "<!-- bounds:end -->"
    for name, lang, full in (("index.html", None, False), ("data.html", None, True), ("paper.html", "en", True), ("paper.ja.html", "ja", True)):
        p = ROOT / "docs" / name
        if not p.exists():
            continue
        txt = p.read_text(encoding="utf-8")
        for lg in (["en", "ja"] if lang is None else [lang]):
            aa, zz = a.replace("start", "start:" + lg), z.replace("end", "end:" + lg)
            if aa in txt and zz in txt:
                txt = txt[: txt.index(aa) + len(aa)] + "\n" + table(lg, full) + "\n    " + txt[txt.index(zz):]
        p.write_text(txt, encoding="utf-8")
    print("docs/data を作り直した。不一致", bad)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
