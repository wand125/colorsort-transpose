"""12 色 272 手解の序盤を一般化して作る。
1. 一番上の色 N を空レーン 0 へ集める（N 手）。
2. 小さなビームで、2 本のレーンを [1..k1] と [1..k2] まで崩す（同色の上・大きい色の上に置く手だけ）。
3. [1..k2] を [1..k1] へ注ぎ、[1..k1, k2..1] と空レーンを作る（k2 手）。
受け側の空きは N−2 マスなので k1+k2 ≥ N、注いだ高さから k1+k2 ≤ N。よって k1+k2 = N
（偶数なら k1=k2=N/2、奇数なら k1=(N+1)/2, k2=(N−1)/2）。
出力: 1 行に 1 候補「k2 手数 手順(36 進)」。
使い方: opening.py N [候補数] [ビーム幅]"""
import sys, heapq
D="0123456789abcdefghijklmnopqrstuvwxyz"
n=int(sys.argv[1]); ncand=int(sys.argv[2]) if len(sys.argv)>2 else 5; W=int(sys.argv[3]) if len(sys.argv)>3 else 3000
k1=(n+1)//2; k2=n//2
base1=tuple(range(1,k1+1)); base2=tuple(range(1,k2+1))
L=[[]]+[list(range(1,n+1)) for _ in range(n)]
pre=[]
for i in range(1,n+1):
    L[0].append(L[i].pop()); pre.append((i,0))
def h(L):
    s=0
    for l in L:
        if not l: continue
        r=1
        while r<len(l) and l[r]==l[0]: r+=1
        s+=len(l)-r
    return s
def prefix_ok(l,k):
    m=0
    while m<len(l) and m<k and l[m]==m+1: m+=1
    return m==k
def score(L,g):
    # 崩し候補: 底が [1..k2] のレーンのうち、上の余分が少ない 2 本（片方は k1 まで残す）
    ex=sorted(len(l)-k2 for l in L[1:] if len(l)>=k2 and prefix_ok(l,k2))
    if len(ex)<2: return 10**9
    return g*10 + (max(ex[0]-(k1-k2),0)+ex[1])*12 + h(L)
def key(L): return tuple(sorted(tuple(l) for l in L))
start=[l[:] for l in L]
layer=[(score(start,0),start,[])]
seen={key(start)}
found=[]
for depth in range(1,4*n):
    nxt=[]
    for _,S,mv in layer:
        for a in range(1,n+1):
            if not S[a]: continue
            c=S[a][-1]
            for b in range(1,n+1):
                if a==b or len(S[b])>=n: continue
                if S[b] and S[b][-1]<c: continue          # 小さい色の上には置かない（R2 型）
                T=[l[:] for l in S]; T[b].append(T[a].pop())
                kk=key(T)
                if kk in seen: continue
                seen.add(kk)
                m2=mv+[(a,b)]
                d1=[i for i in range(1,n+1) if tuple(T[i])==base1]
                d2=[i for i in range(1,n+1) if tuple(T[i])==base2 and i not in d1[:1]]
                if d1 and d2 and d1[0]!=d2[0]:
                    found.append((len(m2),m2,(d2[0],d1[0]),T)); continue
                nxt.append((score(T,len(m2)),T,m2))
    if len(found)>=ncand*3: break
    nxt.sort(key=lambda x:x[0]); layer=nxt[:W]
    if not layer: break
found.sort(key=lambda x:(x[0],h(x[3])))
out=0
for g,m2,(x,y),T in found:
    seq=pre+m2+[(x,y)]*k2
    print(k2, len(seq), "".join(D[a]+D[b] for a,b in seq)); out+=1
    if out>=ncand: break
