"""D の強化版 D' の机上検証。D は段ごとに「そのレーンに都合のよい色」を base+k−1 色選ぶが、
D' は「どのレーンがどの色の家か」を通しで 1 つに決める（残したレーンは底の色の家にしかなれない、色の家は 1 本）。"""
import sys, random, itertools
from functools import lru_cache
from collections import Counter, defaultdict
def analyze(lanes, n):
    base=0; dirty=[]
    for l in lanes:
        if not l: base+=1; continue
        r=1
        while r<len(l) and l[r]==l[0]: r+=1
        if r==len(l): base+=1; continue
        dirty.append((l[0], r, Counter(l[r:]), Counter(l)))
    return base, dirty
def dprime(lanes, n):
    base, dirty = analyze(lanes, n); d=len(dirty)
    if d==0: return 0
    full=(1<<d)-1
    @lru_cache(None)
    def go(mask, O):
        if mask==full: return 0
        best=10**9
        for i in range(d):
            if mask>>i&1: continue
            c0,r,ck,cf=dirty[i]
            if not (O>>c0&1):   # 残す: 底の色の家になる
                cost=sum(v for c,v in ck.items() if not (O>>c&1))
                best=min(best, cost+go(mask|1<<i, O|1<<c0))
            cost=sum(v for c,v in cf.items() if not (O>>c&1))
            rest=[c for c in range(1,n+1) if not (O>>c&1)]
            if rest:
                for c in rest: best=min(best, cost+go(mask|1<<i, O|1<<c))
            else: best=min(best, cost+go(mask|1<<i, O))
        return best
    cols=range(1,n+1); k=min(base,n)
    return min(go(0, sum(1<<c for c in S)) for S in itertools.combinations(cols,k))
if __name__=="__main__":
    fn,n,T=sys.argv[1],int(sys.argv[2]),int(sys.argv[3]); random.seed(1)
    rows=[l.split() for l in open(fn)]
    onp=[r for r in rows if int(r[0])+int(r[1])==T]
    samp=onp+random.sample(rows, min(6000,len(rows)))
    viol=0; gain=Counter(); byg=defaultdict(list); worst=None
    for r in samp:
        g,dd,h,D=map(int,r[:4]); lanes=[tuple(int(ch) for ch in x) if x!="." else () for x in r[4:]]
        while len(lanes)<n+1: lanes.append(())
        Dp=dprime(lanes,n)
        assert Dp>=D,(r,Dp)
        if h+Dp>dd: viol+=1; print("違反",r,Dp)
        gain[Dp-D]+=1
        if g+dd==T: byg[g].append((dd-h-D, dd-h-Dp))
    print(fn,"件数",len(samp),"違反",viol,"D'−D の分布",sorted(gain.items()))
    print(" 最短経路上（深さ: 件数, 差の平均 D → D'）:")
    print("  ",[(g,len(v),round(sum(a for a,b in v)/len(v),2),round(sum(b for a,b in v)/len(v),2)) for g,v in sorted(byg.items())])
