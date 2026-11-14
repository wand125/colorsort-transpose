"""序盤の手順のあとをビームで解き、全体の手順を検証して出す。
使い方: fromopen.py N 序盤手順 幅 [ビームの追加オプション…]（WSL の ~/colorsort3 で実行）
出力: 「total=手数 verified=…」と「SOL 手順」"""
import sys, subprocess
D="0123456789abcdefghijklmnopqrstuvwxyz"
n=int(sys.argv[1]); s=sys.argv[2]; w=sys.argv[3]; extra=sys.argv[4:]
mv=[(D.index(s[i]),D.index(s[i+1])) for i in range(0,len(s),2)]
L=[[]]+[list(range(1,n+1)) for _ in range(n)]
for a,b in mv: L[b].append(L[a].pop())
st=",".join("".join(D[x] for x in l) or "." for l in L)
out=subprocess.run(["nice","-n","10",__import__("os").environ.get("BIN","./target/release/colorsort"),str(n),w,"--start",st]+extra,capture_output=True,text=True).stdout.split('\n')
if not out[0].startswith("N="): print("NOT FOUND"); sys.exit()
rest=out[1].strip()
seq=mv+[(D.index(rest[i]),D.index(rest[i+1])) for i in range(0,len(rest),2)]
L=[[]]+[list(range(1,n+1)) for _ in range(n)]
ok=True
for a,b in seq:
    if not L[a] or len(L[b])>=n: ok=False; break
    L[b].append(L[a].pop())
ok=ok and all(not l or (len(l)==n and len(set(l))==1) for l in L)
print(f"N={n} opening={len(mv)} total={len(seq)} verified={ok}",flush=True)
print("SOL","".join(D[a]+D[b] for a,b in seq),flush=True)
