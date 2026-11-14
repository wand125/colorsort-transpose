from collections import defaultdict
def prec(rows,T,fn):
    """各 g で最適経路上の状態数 k を数え、スコア上位 k 件に入る最適状態の割合（precision@k）"""
    by=defaultdict(list)
    for r in rows: by[r['gs']].append(r)
    tot=hit=0
    for g,rs in by.items():
        k=sum(1 for r in rs if r['gs']+r['dg']==T)
        if k==0 or k==len(rs): continue
        rs=sorted(rs,key=lambda r:(fn(r),r['dg']*0))
        hit+=sum(1 for r in rs[:k] if r['gs']+r['dg']==T); tot+=k
    return hit/tot
