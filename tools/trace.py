"""36 進の手順を、同じ手をまとめて盤面付きで表示する。使い方: trace.py N 手順文字列"""
import sys
D="0123456789abcdefghijklmnopqrstuvwxyz"
n=int(sys.argv[1]); s=sys.argv[2]
mv=[(D.index(s[i]),D.index(s[i+1])) for i in range(0,len(s),2)]
L=[[]]+[list(range(1,n+1)) for _ in range(n)]
i=0; t=0
while i<len(mv):
    a,b=mv[i]; k=1
    while i+k<len(mv) and mv[i+k]==(a,b): k+=1
    for _ in range(k): L[b].append(L[a].pop())
    t+=k; i+=k
    print(f"{t:4d} {D[a]}->{D[b]}"+(f" x{k}" if k>1 else "    ")+"  "+" ".join("".join(D[x] for x in l) or "." for l in L))
