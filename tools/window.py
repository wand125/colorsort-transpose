"""区間の解き直しの試作（机上検討用）。解の t1..t2 手目を、レーンの入れ替えを同一視した上で厳密に最短にできるか調べる。
下界 h(s→目標): 両者で同じレーンを打ち消し、残りを割り当て（少数なら全順列、多ければ各レーンの最小）で Σ(高さ − 共通の底の長さ)。"""
import sys, itertools, heapq, time
from collections import Counter
D="0123456789abcdefghijklmnopqrstuvwxyz"
import os
R=os.path.join(os.path.dirname(os.path.abspath(__file__)),"..","solutions","")
def load():
    sols={}
    for fn in ("best.tsv","docs_known.tsv"):
        for line in open(R+fn).read().split("\n")[1:]:
            if line:
                f=line.split("\t"); sols[(int(f[0]),len(f[-1])//2)]=f[-1]
    return sols
def states(n,s):
    L=[()]+[tuple(range(1,n+1)) for _ in range(n)]; out=[tuple(sorted(L))]
    for i in range(0,len(s),2):
        a,b=D.index(s[i]),D.index(s[i+1]); L[b]=L[b]+(L[a][-1],); L[a]=L[a][:-1]; out.append(tuple(sorted(L)))
    return out
def lcp(a,b):
    k=0
    for x,y in zip(a,b):
        if x!=y: break
        k+=1
    return k
def hdist(s,t):
    cs,ct=Counter(s),Counter(t); a=list((cs-ct).elements()); b=list((ct-cs).elements())
    if not a: return 0
    if len(a)<=6:
        c=[[len(x)-lcp(x,y) for y in b] for x in a]
        return min(sum(c[i][p[i]] for i in range(len(a))) for p in itertools.permutations(range(len(a))))
    return max(sum(min(len(x)-lcp(x,y) for y in b) for x in a), sum(min(len(y)-lcp(x,y) for x in a) for y in b))
def nbrs(s,n):
    seen=set()
    for i,a in enumerate(s):
        if not a or (i and s[i-1]==a): continue
        x=a[-1]
        for j,b in enumerate(s):
            if i==j or len(b)>=n or (j and s[j-1]==b and j-1!=i): continue
            L=list(s); L[i]=a[:-1]; L[j]=b+(x,); t=tuple(sorted(L))
            if t not in seen: seen.add(t); yield t
def shortest(s,t,n,bound,cap):
    """s→t の最短を A* で。bound 以下の経路だけ探す。戻り値 (長さ or None, 展開数, 打ち切りか)"""
    h0=hdist(s,t)
    if h0>bound: return None,0,False
    pq=[(h0,0,s)]; best={s:0}; ex=0
    while pq:
        f,g,u=heapq.heappop(pq)
        if u==t: return g,ex,False
        if best.get(u,1e9)<g: continue
        ex+=1
        if ex>cap: return None,ex,True
        for v in nbrs(u,n):
            g2=g+1
            if best.get(v,1e9)<=g2: continue
            f2=g2+hdist(v,t)
            if f2>bound: continue
            best[v]=g2; heapq.heappush(pq,(f2,g2,v))
    return None,ex,False
if __name__=="__main__":
    n,T,Lw=int(sys.argv[1]),int(sys.argv[2]),int(sys.argv[3]); cap=int(sys.argv[4]) if len(sys.argv)>4 else 20000
    S=states(n,load()[(n,T)]); t0=time.time(); found=[]; cut=0; cand=0; exs=0
    for t1 in range(0,T-Lw+1):
        t2=t1+Lw; h=hdist(S[t1],S[t2])
        if h>Lw-1: continue
        cand+=1
        d,ex,c=shortest(S[t1],S[t2],n,Lw-1,cap); exs+=ex; cut+=c
        if d is not None: found.append((t1,t2,d))
    print(f"N={n} T={T} 窓={Lw}: 下界で残る窓 {cand}/{T-Lw+1}、打ち切り {cut}、短縮できる窓 {len(found)}、展開 {exs}、{time.time()-t0:.1f}s")
    print("  ",found[:12])
