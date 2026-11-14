"""下界 J の机上検証: h と D' を別々に最小にせず、「どのレーンを残すか・どの色の家がいつ開くか」を 1 つに決めて
（動く玉の数）+（1 回では済まない玉の数）を一緒に最小にする。単色レーン（完成済みを含む）は、残すなら自分の色の家、
残さないなら他の汚れレーンと同じく順番に片付ける。空レーンだけが最初から自由な家になる。"""
import sys, itertools, time
from functools import lru_cache
from collections import Counter
def J(lanes, n):
    e=sum(1 for l in lanes if not l); items=[]; uni=[]
    for l in lanes:
        if not l: continue
        r=1
        while r<len(l) and l[r]==l[0]: r+=1
        if r==len(l): uni.append((l[0],len(l)))
        else: items.append((l[0], len(l)-r, Counter(l[r:]), len(l), Counter(l), True))
    best=10**9; allc=list(range(1,n+1))
    for K in range(1<<len(uni)):
        kept=[uni[i][0] for i in range(len(uni)) if K>>i&1]
        if len(set(kept))<len(kept): continue
        its=items+[(u,0,Counter(),r,Counter({u:r}),False) for i,(u,r) in enumerate(uni) if not K>>i&1]
        d=len(its); full=(1<<d)-1
        @lru_cache(None)
        def go(mask,O):
            if mask==full: return 0
            b=10**9
            for i in range(d):
                if mask>>i&1: continue
                c0,nk,ck,nf,cf,cankeep=its[i]; nm=mask|1<<i
                if cankeep and not O>>c0&1:
                    b=min(b, nk+sum(v for c,v in ck.items() if not O>>c&1)+go(nm,O|1<<c0))
                cost=nf+sum(v for c,v in cf.items() if not O>>c&1)
                if cost>=b: continue
                b=min(b,cost+go(nm,O))
                for c in allc:
                    if not O>>c&1: b=min(b,cost+go(nm,O|1<<c))
            return b
        K0=sum(1<<c for c in kept); rest=[c for c in allc if c not in kept]
        for k in range(0,min(e,len(rest))+1):
            for S in itertools.combinations(rest,k):
                best=min(best, go(0,K0|sum(1<<c for c in S)))
    return best
if __name__=="__main__":
    import os; D=os.path.join(os.path.dirname(os.path.abspath(__file__)),"..","learn","")
    sys.path.insert(0,os.path.dirname(os.path.abspath(__file__))); import dprime
    for fn,n,T in (("dump4.txt",4,21),("band5.txt",5,36)):
        t0=time.time(); viol=0; gain=Counter(); gap=Counter(); gapp=Counter(); k=0; onp={}
        for l in open(D+fn):
            r=l.split(); g,dd,h,Dv=map(int,r[:4]); lanes=[tuple(int(ch) for ch in x) if x!="." else () for x in r[4:]]
            while len(lanes)<n+1: lanes.append(())
            j=J(lanes,n); dp=h+dprime.dprime(lanes,n); k+=1
            if j>dd: viol+=1; print("違反",l.strip(),j) if viol<5 else None
            if j<dp: print("J<h+D'",l.strip(),j,dp)
            gain[j-dp]+=1; gap[dd-j]+=1; gapp[dd-dp]+=1
            if g+dd==T: onp.setdefault(g,[]).append((dd-dp,dd-j))
        print(fn,"全",k,"件 違反",viol,f"{time.time()-t0:.0f}s")
        print("  J−(h+D'):",sorted(gain.items()))
        print("  距離との差 h+D':",sorted(gapp.items()),"平均",round(sum(a*b for a,b in gapp.items())/k,2))
        print("  距離との差 J   :",sorted(gap.items()),"平均",round(sum(a*b for a,b in gap.items())/k,2))
        print("  最短経路上:",[(g,round(sum(a for a,b in v)/len(v),1),round(sum(b for a,b in v)/len(v),1)) for g,v in sorted(onp.items())])
