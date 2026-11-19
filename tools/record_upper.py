"""上界の更新を記録する: record_upper.py N SEQFILE [OLD=NEW ...]
- 手順を replay で確かめる。
- solutions/best.tsv の N 行の手数と手順、bounds.tsv の上界を置き換える。
- README.md・docs/paper*.html の表の N 行の上界を置き換える。
- 追加の "OLD => NEW" は、README.md・docs/paper*.html の本文の文字列置換（例: "gives 635 => gives 633"）。
  どれも 1 か所以上当たらなければ止まる。
そのあと verify_all.py --readme と build_site.py を回すこと。"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import verify_all as v

root = Path(__file__).resolve().parents[1]

n = int(sys.argv[1])
seq = Path(sys.argv[2]).read_text().split()[-1]
new = len(seq) // 2
assert v.replay(n, seq), "replay 失敗"
pairs = [a.split(" => ", 1) for a in sys.argv[3:]]
assert all(len(p) == 2 for p in pairs), "置換は \"OLD => NEW\" の形で"
for a, _ in pairs:  # 書き換える前に、どの置換も当たることを確かめる
    assert any(a in (root / fn).read_text() for fn in ["README.md", "docs/paper.html", "docs/paper.ja.html"]), f"当たらない: {a}"

bt = root / "solutions/best.tsv"
bp = root / "bounds.tsv"
# 今の上界は bounds.tsv の値（best.tsv の行は記録でない手順のこともある。例: 8・12 色は docs_known.tsv が記録）。
# どちらも書き換える前に確かめる。
BL = bp.read_text().split("\n")
old = next((int(l.split("\t")[3]) for l in BL if l.split("\t")[0] == str(n)), None)
assert old is not None and new < old, (old, new)
L = bt.read_text().split("\n")
hit = False
for i, l in enumerate(L):
    f = l.split("\t")
    if f[0] == str(n):
        f[1] = str(new)
        f = [seq if len(x) > 200 else x for x in f]
        L[i] = "\t".join(f)
        hit = True
assert hit, "best.tsv に N の行がない"
bt.write_text("\n".join(L))
for i, l in enumerate(BL):
    f = l.split("\t")
    if f[0] == str(n):
        f[3] = str(new)
        if len(f) > 5 and f[5] != "solutions/best.tsv":
            f[5] = "solutions/best.tsv"
        BL[i] = "\t".join(f)
bp.write_text("\n".join(BL))

for fn in ["README.md", "docs/paper.html", "docs/paper.ja.html"]:
    p = root / fn
    s = p.read_text()
    row_md = f"| {n} | "
    lines = s.split("\n")
    for k, ln in enumerate(lines):
        if ln.startswith(row_md):
            lines[k] = ln.replace(f"| {old} |", f"| {new} |", 1)
        if f'<td class="num">{n}</td>' in ln:
            lines[k] = ln.replace(f'<td class="num">{old}</td>', f'<td class="num">{new}</td>', 1)
    s = "\n".join(lines)
    p.write_text(s)
for a, b in pairs:
    hit = 0
    for fn in ["README.md", "docs/paper.html", "docs/paper.ja.html"]:
        p = root / fn
        s = p.read_text()
        if a in s:
            hit += 1
            p.write_text(s.replace(a, b))
    assert hit, f"当たらない: {a}"
print(f"N={n}: {old} -> {new}")
