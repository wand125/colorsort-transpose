import sys
D="0123456789abcdefghijklmnopqrstuvwxyz"
lines=open('r1r2.txt').read().split('\n')
sols={}
for i,l in enumerate(lines):
    if l.startswith('N='):
        n=int(l.split()[0][2:]); sols[n]=lines[i+1].strip()
def run(n,s,show_every=False):
    mv=[(D.index(s[i]),D.index(s[i+1])) for i in range(0,len(s),2)]
    lanes=[[]]+[list(range(1,n+1)) for _ in range(n)]
    out=[]
    # 連続する同じ (a,b) をまとめて表示
    i=0
    while i<len(mv):
        a,b=mv[i]; k=1
        while i+k<len(mv) and mv[i+k]==(a,b): k+=1
        for _ in range(k): lanes[b].append(lanes[a].pop())
        out.append(f"{a}->{b}"+(f" x{k}" if k>1 else "")+"   "+" | ".join("".join(map(str,l)) or "." for l in lanes))
        i+=k
    return out
n=int(sys.argv[1])
print("\n".join(run(n,sols[n])))
