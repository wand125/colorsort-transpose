"""既知解の t 手目の局面から先をビームで解き直し、短くなれば全体の手順を出す。
使い方: suffix.py N 手順 幅 t1,t2,... [ビームの追加オプション…]（WSL の ~/colorsort3 で実行）"""
import sys, subprocess
D="0123456789abcdefghijklmnopqrstuvwxyz"
n=int(sys.argv[1]); s=sys.argv[2]; w=sys.argv[3]; ts=[int(x) for x in sys.argv[4].split(',')]; extra=sys.argv[5:]
mv=[(D.index(s[i]),D.index(s[i+1])) for i in range(0,len(s),2)]
T=len(mv)
def state_at(t):
    L=[[]]+[list(range(1,n+1)) for _ in range(n)]
    for a,b in mv[:t]: L[b].append(L[a].pop())
    return L
def verify(seq):
    L=[[]]+[list(range(1,n+1)) for _ in range(n)]
    for a,b in seq:
        if not L[a] or len(L[b])>=n: return False
        L[b].append(L[a].pop())
    return all(not l or (len(l)==n and len(set(l))==1) for l in L)
for t in ts:
    L=state_at(t)
    st=",".join("".join(D[x] for x in l) or "." for l in L)
    out=subprocess.run(["nice","-n","10",__import__("os").environ.get("BIN","./target/release/colorsort"),str(n),w,"--start",st]+extra,capture_output=True,text=True).stdout.split('\n')
    if not out[0].startswith("N="): print(t,"NOT FOUND",flush=True); continue
    rest=out[1].strip(); r=len(rest)//2
    seq=mv[:t]+[(D.index(rest[i]),D.index(rest[i+1])) for i in range(0,len(rest),2)]
    tot=t+r
    print(f"t={t} rest={r} (元 {T-t}) total={tot} verified={verify(seq)}"+("  ★改善" if tot<T else ""),flush=True)
    if tot<T and verify(seq):
        print("SOL", "".join(D[a]+D[b] for a,b in seq),flush=True)
