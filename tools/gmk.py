import itertools,subprocess,sys
from collections import Counter
n=int(sys.argv[1]); m=int(sys.argv[2]); k=n-m
balls=[c for c in range(1,m+1) for _ in range(k)]
seen=set(); res=Counter(); ex={}
for p in set(itertools.permutations(balls)):
    lanes=tuple(sorted("".join(map(str,range(1,m+1)))+"".join(map(str,p[i*k:(i+1)*k])) for i in range(m)))
    if lanes in seen: continue
    seen.add(lanes)
    args=["."]+list(lanes)+[str(c)*n for c in range(m+1,n+1)]
    out=subprocess.run(["nice","-n","10","./target/release/colorlb","opt",str(n)]+args,capture_output=True,text=True).stdout
    g=int(out.strip().split()[-1]); res[g]+=1; ex.setdefault(g,lanes)
print("N",n,"m",m,"target",(2*n-1)*(m-1),dict(sorted(res.items())),{g:ex[g] for g in sorted(ex)})
