"""解き直しを反復する。手順の途中 t（全体の 15%〜85% を等間隔）から先をビームで解き直し、
短くなったら採用して最初からやり直す。改善がなくなるまで続け、各改善を best_N.txt に追記する。
使い方: iterate.py N 手順 幅 [分割数] （WSL の ~/colorsort3 で実行）"""
import sys, subprocess
D="0123456789abcdefghijklmnopqrstuvwxyz"
n=int(sys.argv[1]); s=sys.argv[2]; w=sys.argv[3]; parts=int(sys.argv[4]) if len(sys.argv)>4 else 8
def parse(s): return [(D.index(s[i]),D.index(s[i+1])) for i in range(0,len(s),2)]
def verify(seq):
    L=[[]]+[list(range(1,n+1)) for _ in range(n)]
    for a,b in seq:
        if not L[a] or len(L[b])>=n: return False
        L[b].append(L[a].pop())
    return all(not l or (len(l)==n and len(set(l))==1) for l in L)
mv=parse(s); assert verify(mv)
tried=set()
improved=True
while improved:
    improved=False
    T=len(mv)
    for j in range(1,parts):
        t=round(T*(0.15+0.7*(j-1)/(parts-2))) if parts>2 else T//2
        key=(T,t)
        if key in tried: continue
        tried.add(key)
        L=[[]]+[list(range(1,n+1)) for _ in range(n)]
        for a,b in mv[:t]: L[b].append(L[a].pop())
        st=",".join("".join(D[x] for x in l) or "." for l in L)
        out=subprocess.run(["nice","-n","10","./target/release/colorsort",str(n),w,"--start",st],capture_output=True,text=True).stdout.split('\n')
        if not out[0].startswith("N="): continue
        seq=mv[:t]+parse(out[1].strip())
        print(f"N={n} T={T} t={t} -> {len(seq)}",flush=True)
        if len(seq)<T and verify(seq):
            mv=seq; improved=True
            with open(f"best_{n}.txt","a") as f: f.write(f"{len(mv)}\t"+"".join(D[a]+D[b] for a,b in mv)+"\n")
            print(f"★ N={n} 改善 {T} -> {len(mv)}",flush=True)
            break
print(f"N={n} 最終 {len(mv)}",flush=True)
