"""dump の行（g_s d_g h D レーン…）から特徴量を作る。"""
def lanes_of(ws):
    return [[int(c,16) for c in w] if w!='.' else [] for w in ws]
def feats(L, n):
    f={}
    # P: 初期の並び 1,2,3,… が残る量
    ks=[]
    for l in L:
        k=0
        while k<len(l) and l[k]==k+1: k+=1
        ks.append(k)
    f['P']=sum(ks)-max(ks)
    f['E']=sum(1 for l in L if not l)
    f['done']=sum(1 for l in L if len(l)==n and len(set(l))==1)
    asc=desc=same=0
    for l in L:
        for a,b in zip(l,l[1:]):
            if b>a: asc+=1
            elif b<a: desc+=1
            else: same+=1
    f['asc']=asc; f['desc']=desc; f['same']=same
    # 上に同色が続く塊の数（運ぶ単位）
    f['runs']=sum(sum(1 for i in range(len(l)) if i==0 or l[i]!=l[i-1]) for l in L)
    # 色ごとに散らばるレーン数
    f['spread']=sum(len({i for i,l in enumerate(L) if c in l})-1 for c in range(1,n+1))
    # 底の色の重複（同じ底色のレーンが複数）
    from collections import Counter
    bc=Counter(l[0] for l in L if l)
    f['botdup']=sum(v-1 for v in bc.values())
    # 埋もれ: 各玉の上にある「別の色」の玉の数の合計
    f['bury']=sum(sum(1 for y in l[i+1:] if y!=l[i]) for l in L for i in range(len(l)))
    # 空きのあるレーンの一番上の色の種類（置き場の多様さ）
    f['tops']=len({l[-1] for l in L if l and len(l)<n})
    return f
def load(path, n):
    rows=[]
    for line in open(path):
        p=line.split()
        gs,dg,h,D=map(int,p[:4]); L=lanes_of(p[4:])
        f=feats(L,n); f.update(gs=gs,dg=dg,h=h,D=D); rows.append(f)
    return rows
