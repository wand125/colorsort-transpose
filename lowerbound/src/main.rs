//! 色そろえパズルの下界 h + D の検証と、下界の証明探索。
//!
//! D(s): 「2 回以上動く玉」の数の下界（Docs 4 節の数え方を途中の状態に広げたもの）。
//!   - レーンを 空 / 単色 / 汚れ（底の同色部分の上に別の玉がある）に分ける。
//!   - 汚れたレーンは、底の色の「家」として残す（keep: 上の部分だけ出す）か、
//!     全部出す（full）。底の色ごとに残せるのは 1 本まで（単色レーンが残る場合は 0 本）。
//!   - 汚れたレーンを片付け終わる順に並べると、k 番目のレーンから玉を出すとき、
//!     きれいなレーン（色の家になれる）は多くても base + k − 1 本（base = 空 + 単色）。
//!     出す玉のうち 1 回で済むのは、色ごとの個数の多い順に base + k − 1 色分まで。
//!   - 並べ方と残すレーンの選び方で最小を取る（部分集合 DP）。
//!
//! 使い方:
//!   colorlb check N            N≤4: 全状態の正確な距離と比べる
//!   colorlb near N DEPTH       完成形から DEPTH 手以内の状態で比べる
//!   colorlb prove N B          g + h + D ≤ B−1 の状態だけを幅優先でたどり、B−1 手以内の解が無いか調べる
//!   colorlb j1file N FILE       compare J and J1 with Python per-line references
//!   colorlb init N             初期状態の h と D
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

const MAXL: usize = 13;
type Lane = u64;
type State = [Lane; MAXL];

#[inline]
fn height(l: Lane) -> usize { (l >> 60) as usize }
#[inline]
fn ball(l: Lane, k: usize) -> u8 { ((l >> (4 * k)) & 15) as u8 }
#[inline]
fn top(l: Lane) -> u8 { ball(l, height(l) - 1) }
#[inline]
fn pop(l: Lane) -> Lane {
    let h = height(l);
    (l & ((1u64 << (4 * (h - 1))) - 1)) | (((h - 1) as u64) << 60)
}
#[inline]
fn push(l: Lane, c: u8) -> Lane {
    let h = height(l);
    (l & ((1u64 << 60) - 1)) | ((c as u64) << (4 * h)) | (((h + 1) as u64) << 60)
}

fn initial(n: usize) -> State {
    let mut s = [0u64; MAXL];
    for i in 1..=n {
        for c in 1..=n {
            s[i] = push(s[i], c as u8);
        }
    }
    s
}
fn goal(n: usize) -> State {
    let mut s = [0u64; MAXL];
    for i in 1..=n {
        for _ in 0..n {
            s[i] = push(s[i], i as u8);
        }
    }
    canon(&s, n + 1)
}
fn canon(s: &State, lanes: usize) -> State {
    let mut t = *s;
    t[..lanes].sort_unstable();
    t
}
fn is_goal(s: &State, n: usize) -> bool {
    s[..=n].iter().all(|&l| {
        let h = height(l);
        h == 0 || (h == n && (0..h).all(|k| ball(l, k) == ball(l, 0)))
    })
}
fn apply(s: &State, a: usize, b: usize) -> State {
    let mut t = *s;
    let c = top(t[a]);
    t[a] = pop(t[a]);
    t[b] = push(t[b], c);
    t
}
/// 正規形の隣接状態（重複は呼び出し側で除く）。空レーンへは 1 本だけ、1 個だけのレーンから空レーンへは打たない。
fn neighbors(s: &State, n: usize, out: &mut Vec<State>) {
    out.clear();
    let lanes = n + 1;
    let first_empty = (0..lanes).find(|&i| height(s[i]) == 0);
    for a in 0..lanes {
        let ha = height(s[a]);
        if ha == 0 { continue; }
        for b in 0..lanes {
            if a == b { continue; }
            let hb = height(s[b]);
            if hb >= n { continue; }
            if hb == 0 && (Some(b) != first_empty || ha == 1) { continue; }
            out.push(canon(&apply(s, a, b), lanes));
        }
    }
}

/// 下界 h（Docs 4 節）
fn h1(s: &State, n: usize) -> u32 {
    let mut h = 0u32;
    let mut sum_r = [0u32; 16];
    let mut max_r = [0u32; 16];
    for &l in &s[..=n] {
        let ht = height(l);
        if ht == 0 { continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        h += (ht - r) as u32;
        sum_r[c0 as usize] += r as u32;
        max_r[c0 as usize] = max_r[c0 as usize].max(r as u32);
    }
    for c in 1..=n { h += sum_r[c] - max_r[c]; }
    h
}

/// D の強化版 D'（fork 側の机上検討、tools/dprime.py の移植）。「どの色の家がいつ開くか」を通しで 1 つに決める。
/// DP の状態は（片付いた汚れレーンの集合, 家が開いた色の集合 O）。初期の O は大きさ min(base, N) の全部を試す。
/// 許容性（D' ≤ 2 回以上動く玉の数）の証明は LOWER_BOUND_PROOF.md。N=4 の全状態でも違反 0 を確かめた。prove では D と併用し max を取る。
fn d3(s: &State, n: usize) -> u32 {
    let mut base = 0usize;
    let mut dl: Vec<(u8, [u8; 16], [u8; 16])> = Vec::new();
    for &l in &s[..=n] {
        let ht = height(l);
        if ht == 0 { base += 1; continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        if r == ht { base += 1; continue; }
        let mut keep = [0u8; 16];
        for k in r..ht { keep[ball(l, k) as usize] += 1; }
        let mut full = keep;
        full[c0 as usize] += r as u8;
        dl.push((c0, keep, full));
    }
    let d = dl.len();
    if d == 0 { return 0; }
    let colors: u32 = ((1u32 << (n + 1)) - 1) & !1;
    // メモは呼び出しごとに確保し直さず、スレッドごとの領域を世代番号で使い回す（7・8 色で確保と初期化が重かった）
    thread_local! { static BUF: std::cell::RefCell<(Vec<u32>, Vec<u8>, u32)> = std::cell::RefCell::new((Vec::new(), Vec::new(), 0)); }
    BUF.with(|cell| {
    let mut guard = cell.borrow_mut();
    let (stamp, val, epoch) = &mut *guard;
    let size = (1usize << d) << (n + 1);
    if stamp.len() < size { stamp.resize(size, 0); val.resize(size, 0); }
    *epoch = epoch.wrapping_add(1);
    if *epoch == 0 { stamp.iter_mut().for_each(|x| *x = 0); *epoch = 1; }
    let ep = *epoch;
    let mut memo = Memo { stamp, val, ep };
    fn cost(cnt: &[u8; 16], o: u32, n: usize) -> u32 {
        let mut c = 0u32;
        for k in 1..=n { if o >> k & 1 == 0 { c += cnt[k] as u32; } }
        c
    }
    struct Memo<'a> { stamp: &'a mut Vec<u32>, val: &'a mut Vec<u8>, ep: u32 }
    fn go(mask: usize, o: u32, d: usize, n: usize, colors: u32, dl: &[(u8, [u8; 16], [u8; 16])], memo: &mut Memo) -> u32 {
        if mask == (1 << d) - 1 { return 0; }
        let key = (mask << (n + 1)) | o as usize;
        if memo.stamp[key] == memo.ep { return memo.val[key] as u32; }
        let mut best = u32::MAX;
        for i in 0..d {
            if mask >> i & 1 == 1 { continue; }
            let (c0, ref keep, ref full) = dl[i];
            let nm = mask | 1 << i;
            if o >> c0 & 1 == 0 {
                let v = cost(keep, o, n) + go(nm, o | 1 << c0, d, n, colors, dl, memo);
                if v < best { best = v; }
            }
            let fc = cost(full, o, n);
            if fc >= best { continue; }
            let mut rest = colors & !o;
            if rest == 0 {
                let v = fc + go(nm, o, d, n, colors, dl, memo);
                if v < best { best = v; }
            }
            while rest != 0 {
                let c = rest.trailing_zeros();
                rest &= rest - 1;
                let v = fc + go(nm, o | 1 << c, d, n, colors, dl, memo);
                if v < best { best = v; }
            }
        }
        memo.stamp[key] = memo.ep;
        memo.val[key] = best.min(254) as u8;
        best
    }
    let k = base.min(n) as u32;
    let mut best = u32::MAX;
    let mut o = 0u32;
    while o <= colors {
        if o & 1 == 0 && o & !colors == 0 && o.count_ones() == k {
            let v = go(0, o, d, n, colors, &dl, &mut memo);
            if v < best { best = v; }
        }
        o += 2;
    }
    best
    })
}
fn use_dprime() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_DPRIME").map(|v| v == "1").unwrap_or(false))
}

/// さらに強い下界 J（fork 側の設計、tools/joint.py の移植。証明は LOWER_BOUND_PROOF.md「さらに強い下界 J」）。
/// h + D′ と違い、これ単独で「残り手数の下界」。単色レーンは「残す（自分の色の家）」か「順番に片付ける」のどちらか。
/// 最初から自由な家は空レーンだけで、e 本なら 0〜e 色を選ぶ。「全部出す」は 1 色足すか何も足さない（両方試す）。
fn jlb(s: &State, n: usize) -> u32 {
    // (底の色, 残すときの動く数, 残すときの表, 全部出すときの数, 全部の表, 残せるか)
    type It = (u8, u8, [u8; 16], u8, [u8; 16], bool);
    let mut e = 0usize;
    let mut items: Vec<It> = Vec::new();
    let mut uni: Vec<(u8, u8)> = Vec::new();
    for &l in &s[..=n] {
        let ht = height(l);
        if ht == 0 { e += 1; continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        // 完成したレーン（色 c0 が N 個）は残すのが常に最善: 費用 0 で、ほかのレーンに c0 は無いので何も妨げない。
        // 片付けるなら N 手以上かかる。値を変えずに外せる（2^単色レーン数 の繰り返しが減る）。
        if r == ht && ht == n { continue; }
        if r == ht { uni.push((c0, ht as u8)); continue; }
        let mut keep = [0u8; 16];
        for k in r..ht { keep[ball(l, k) as usize] += 1; }
        let mut full = keep;
        full[c0 as usize] += r as u8;
        items.push((c0, (ht - r) as u8, keep, ht as u8, full, true));
    }
    let colors: u32 = ((1u32 << (n + 1)) - 1) & !1;
    struct Memo<'a> { stamp: &'a mut Vec<u32>, val: &'a mut Vec<u8>, ep: u32 }
    fn cost(cnt: &[u8; 16], o: u32, n: usize) -> u32 {
        let mut c = 0u32;
        for k in 1..=n { if o >> k & 1 == 0 { c += cnt[k] as u32; } }
        c
    }
    // pm[i]: レーン i の全部の表に出てくる色。残りのレーンに出てこない色を家に足しても、費用も「残せるか」も変わらない
    // （底の色 c0 のレーンが残っていれば c0 は表に出る）ので、足す色の候補から外す（最小値は変わらない）。
    // 同じ形のレーンは入れ替えても値が同じなので、形ごとに「何本片付けたか」だけを持つ（gc[g] 本のうち、混合基数 gm[g] の桁）。
    // 桁はビット欄（gm[g] = 1 << 位置、幅は gc[g] が入る最小のビット数）にして、割り算を避ける。
    struct Gr<'a> { its: &'a [It], gc: &'a [usize], gm: &'a [usize], sh: &'a [u32], mk: &'a [usize], pm: &'a [u32], done: usize }
    fn go(code: usize, o: u32, n: usize, colors: u32, gr: &Gr, memo: &mut Memo) -> u32 {
        if code == gr.done { return 0; }
        let (its, pm) = (gr.its, gr.pm);
        let d = its.len();
        let key = (code << (n + 1)) | o as usize;
        if memo.stamp[key] == memo.ep { return memo.val[key] as u32; }
        let mut best = u32::MAX;
        for i in 0..d {
            if code >> gr.sh[i] & gr.mk[i] == gr.gc[i] { continue; }
            let (c0, nk, ref keep, nf, ref full, cankeep) = its[i];
            let nm = code + gr.gm[i];
            if cankeep && o >> c0 & 1 == 0 {
                let v = nk as u32 + cost(keep, o, n) + go(nm, o | 1 << c0, n, colors, gr, memo);
                if v < best { best = v; }
            }
            let fc = nf as u32 + cost(full, o, n);
            if fc >= best { continue; }
            let v = fc + go(nm, o, n, colors, gr, memo);
            if v < best { best = v; }
            let mut present = 0u32;
            for j in 0..d { if nm >> gr.sh[j] & gr.mk[j] < gr.gc[j] { present |= pm[j]; } }
            let mut rest = colors & !o & present;
            while rest != 0 {
                let c = rest.trailing_zeros();
                rest &= rest - 1;
                let v = fc + go(nm, o | 1 << c, n, colors, gr, memo);
                if v < best { best = v; }
            }
        }
        memo.stamp[key] = memo.ep;
        memo.val[key] = best.min(254) as u8;
        best
    }
    thread_local! { static JBUF: std::cell::RefCell<(Vec<u32>, Vec<u8>, u32)> = std::cell::RefCell::new((Vec::new(), Vec::new(), 0)); }
    JBUF.with(|cell| {
        let mut guard = cell.borrow_mut();
        let (stamp, val, epoch) = &mut *guard;
        let mut best = u32::MAX;
        let nu = uni.len();
        for kset in 0usize..(1 << nu) {
            let mut k0 = 0u32;
            let mut ok = true;
            let mut its = items.clone();
            for (i, &(u, len)) in uni.iter().enumerate() {
                if kset >> i & 1 == 1 {
                    if k0 >> u & 1 == 1 { ok = false; break; }
                    k0 |= 1 << u;
                } else {
                    let mut full = [0u8; 16];
                    full[u as usize] = len;
                    its.push((u, 0, [0u8; 16], len, full, false));
                }
            }
            if !ok { continue; }
            its.sort_unstable();
            let mut gi: Vec<It> = Vec::new();
            let mut gc: Vec<usize> = Vec::new();
            for it in its { if gi.last() == Some(&it) { *gc.last_mut().unwrap() += 1; } else { gi.push(it); gc.push(1); } }
            let (mut gm, mut sh, mut mk) = (Vec::new(), Vec::new(), Vec::new());
            let mut bits = 0u32;
            let mut done = 0usize;
            for &c in &gc {
                let w = usize::BITS - c.leading_zeros();
                gm.push(1usize << bits); sh.push(bits); mk.push((1usize << w) - 1);
                done |= c << bits;
                bits += w;
            }
            let tot = done + 1;
            let its = gi;
            let size = tot << (n + 1);
            if stamp.len() < size { stamp.resize(size, 0); val.resize(size, 0); }
            *epoch = epoch.wrapping_add(1);
            if *epoch == 0 { stamp.iter_mut().for_each(|x| *x = 0); *epoch = 1; }
            let mut memo = Memo { stamp: &mut *stamp, val: &mut *val, ep: *epoch };
            let pm: Vec<u32> = its.iter().map(|it| { let mut m = 0u32; for k in 1..=n { if it.4[k] > 0 { m |= 1 << k; } } m }).collect();
            // 空レーンの家: k0 に無い色から 0〜e 色（どのレーンにも出てこない色は足しても同じなので外す）
            let free = colors & !k0 & pm.iter().fold(0, |a, &m| a | m);
            let gr = Gr { its: &its, gc: &gc, gm: &gm, sh: &sh, mk: &mk, pm: &pm, done };
            let mut sub = free;
            loop {
                if (sub.count_ones() as usize) <= e {
                    let v = go(0, k0 | sub, n, colors, &gr, &mut memo);
                    if v < best { best = v; }
                }
                if sub == 0 { break; }
                sub = (sub - 1) & free;
            }
        }
        best
    })
}
/// 閾値つきの J。戻り値 r が thr 以下なら、費用 r の道がある（J ≤ r ≤ thr）。thr を超えるなら J ≥ r（下界）。
/// 深さ優先で、(ここまでの費用) + (残りのレーンの静的な下限) が予算を超える枝を切る。
/// 予算内の道が 1 本見つかった時点で返す。切った枝から分かった下限は (片付けたレーン, 開いた色) ごとに表に残し、
/// 予算が違う呼び出しでも使う（下限はどの予算でも正しい）。
fn jlb_thr(s: &State, n: usize, thr: u32) -> u32 {
    // (底の色, 残すときの動く数, 残すときの表, 全部出すときの数, 全部の表, 残せるか)
    type It = (u8, u8, [u8; 16], u8, [u8; 16], bool);
    let mut e = 0usize;
    let mut items: Vec<It> = Vec::new();
    let mut uni: Vec<(u8, u8)> = Vec::new();
    for &l in &s[..=n] {
        let ht = height(l);
        if ht == 0 { e += 1; continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        // 完成したレーン（色 c0 が N 個）は残すのが常に最善: 費用 0 で、ほかのレーンに c0 は無いので何も妨げない。
        // 片付けるなら N 手以上かかる。値を変えずに外せる（2^単色レーン数 の繰り返しが減る）。
        if r == ht && ht == n { continue; }
        if r == ht { uni.push((c0, ht as u8)); continue; }
        let mut keep = [0u8; 16];
        for k in r..ht { keep[ball(l, k) as usize] += 1; }
        let mut full = keep;
        full[c0 as usize] += r as u8;
        items.push((c0, (ht - r) as u8, keep, ht as u8, full, true));
    }
    let colors: u32 = ((1u32 << (n + 1)) - 1) & !1;
    struct Memo<'a> { stamp: &'a mut Vec<u32>, val: &'a mut Vec<u8>, ep: u32, tt: &'a mut Vec<(u64, [u64; 14], u8)>, ttmask: usize, ttmin: usize, nodes: u64, cps: &'a mut Vec<u32>, cpc: &'a mut Vec<u32>, cep: u32 }
    fn cost(cnt: &[u8; 16], o: u32, n: usize) -> u32 {
        let mut c = 0u32;
        for k in 1..=n { if o >> k & 1 == 0 { c += cnt[k] as u32; } }
        c
    }
    // pm[i]: レーン i の全部の表に出てくる色。残りのレーンに出てこない色を家に足しても、費用も「残せるか」も変わらない
    // （底の色 c0 のレーンが残っていれば c0 は表に出る）ので、足す色の候補から外す（最小値は変わらない）。
    // 同じ形のレーンは入れ替えても値が同じなので、形ごとに「何本片付けたか」だけを持つ（gc[g] 本のうち、混合基数 gm[g] の桁）。
    // 桁はビット欄（gm[g] = 1 << 位置、幅は gc[g] が入る最小のビット数）にして、割り算を避ける。
    struct Gr<'a> { its: &'a [It], gc: &'a [usize], gm: &'a [usize], sh: &'a [u32], mk: &'a [usize], pm: &'a [u32], done: usize, cls: &'a [u32], sg: &'a [u64], zg: &'a [u64], ord: &'a [usize] }
    fn go(code: usize, o: u32, budget: u32, n: usize, colors: u32, gr: &Gr, memo: &mut Memo) -> u32 {
        if code == gr.done { return 0; }
        memo.nodes += 1;
        // 色の対称性: どのレーンの表でも個数が同じで、どのレーンの底の色でもない色どうしは入れ替えても値が同じ。
        // 同じ組の中では「何色開いているか」だけを残し、開いている色を組の小さい番号から詰め直す。
        let mut o = o;
        for &m in gr.cls {
            let k = (o & m).count_ones();
            let mut lo = 0u32;
            let mut mm = m;
            for _ in 0..k { let b = mm & mm.wrapping_neg(); lo |= b; mm &= mm - 1; }
            o = (o & !m) | lo;
        }
        let its = gr.its;
        let d = its.len();
        let key = (code << (n + 1)) | o as usize;
        // 記録は 1 語に「世代（上 24 ビット）と値（下 8 ビット）」をまとめて持つ（1 回の読み出しで済ませる）
        let mut stored = { let w = memo.stamp[key]; if w >> 8 == memo.ep { w & 255 } else { 0 } };
        if stored > budget { return stored; }
        // 呼び出しをまたぐ表: 部分問題の値は「残りのレーンの形の多重集合」と（対称性で詰め直した）o だけで決まる。
        // 表には下限だけを入れる（予算内で打ち切った値は入れない）ので、どの呼び出しで使っても下限のまま。鍵は丸ごと比べる。
        // 鍵（形の符号を整列したもの + o）は、指紋（散らし値）が合ったときと書き込むときにだけ作る。
        let mk_key = |gr: &Gr| -> [u64; 14] {
            let mut tk = [0u64; 14];
            let mut m = 0usize;
            for &i in gr.ord {
                let left = gr.gc[i] - (code >> gr.sh[i] & gr.mk[i]);
                for _ in 0..left { tk[m] = gr.sg[i]; m += 1; }
            }
            tk[13] = o as u64;
            tk
        };
        let mut tslot = usize::MAX;
        let mut tfp = 0u64;
        if memo.ttmin > 0 {
            let mut m = 0usize;
            let mut hsh = 0u64;
            for i in 0..d {
                let left = gr.gc[i] - (code >> gr.sh[i] & gr.mk[i]);
                m += left;
                hsh = hsh.wrapping_add((left as u64).wrapping_mul(gr.zg[i]));
            }
            if m >= memo.ttmin {
                hsh = (hsh ^ (o as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
                hsh ^= hsh >> 31;
                tslot = (hsh as usize) & memo.ttmask;
                tfp = hsh | 1;
                if memo.tt[tslot].0 == tfp && memo.tt[tslot].1 == mk_key(gr) {
                    stored = stored.max(memo.tt[tslot].2 as u32);
                    if stored > budget { return stored; }
                }
            }
        }
        // 残りのレーンの静的な下限: 残せるなら「残す」の動く数、残せないなら全部
        // 底の色の家がもう開いているレーンは「残す」ができないので、下限は全部出す数にする（o が増えるほど大きくなるだけなので、親の側の見積もりも下限のまま）
        let hb = use_hb();
        let lbi = |i: usize| -> u32 { if its[i].5 && (!hb || o >> its[i].0 & 1 == 0) { its[i].1 as u32 } else { its[i].3 as u32 } };
        let mut h = 0u32;
        // 次に片付けるレーンは、静的な下限に加えて「家の開いていない色の玉」の分も払う（開ける色はそのレーン自身が払ったあとに開く）。
        // 残りのレーンのうち、その余分が一番小さいものを足しても下限のまま。
        // 順番 p 番目（0 始まり）に片付けるレーンより前に開く色は高々 p 色。そのレーンの余分（家の開いていない色の玉）は、
        // 開いていない色の個数の多い順に p 色ぶんを引いた値以上。各 p で余分の下限が最小のレーンを取って足しても下限のまま
        // （どの並べ方でも、p 番目のレーンの余分はその最小以上）。
        // レーンの形 i と o だけで決まる値（cp[q]・全部出す費用 xf・残す費用 xk）は、o ごとに 1 回だけ作って覚える（呼び出しの中で使い回す）。
        const CW: usize = 34;
        let cb = o as usize * d * CW;
        if memo.cps[o as usize] != memo.cep {
            for i in 0..d {
                let (c0, nk, ref keep, nf, ref full, cankeep) = its[i];
                let reduce = |t: &[u8; 16], skip: u8| -> [u32; 16] {
                    let mut v: [u8; 16] = [0; 16];
                    let mut m = 0;
                    for k in 1..=n { if o >> k & 1 == 0 && k as u8 != skip && t[k] > 0 { v[m] = t[k]; m += 1; } }
                    v[..m].sort_unstable_by(|a, b| b.cmp(a));
                    let mut pre = [0u32; 16];
                    for q in 0..15 { pre[q + 1] = pre[q] + if q < m { v[q] as u32 } else { 0 }; }
                    pre
                };
                let pf = reduce(full, 0);
                let xf = nf as u32 + cost(full, o, n);
                let kp = cankeep && o >> c0 & 1 == 0;
                let pk = if kp { reduce(keep, c0) } else { [0; 16] };
                let xk = if kp { nk as u32 + cost(keep, o, n) } else { u32::MAX };
                let li = lbi(i);
                let row = &mut memo.cpc[cb + i * CW..cb + (i + 1) * CW];
                for q in 0..=n.min(14) {
                    let mut v = xf - pf[q];
                    if kp { v = v.min(xk - pk[q]); }
                    row[q] = v.saturating_sub(li);
                    // 動く数を除いた余分だけ（同じ底の色のレーンは 1 本しか残せないことを使う下限用）
                    let mut x = xf - nf as u32 - pf[q];
                    if kp { x = x.min(xk - nk as u32 - pk[q]); }
                    row[18 + q] = x;
                }
                row[16] = xf;
                row[17] = xk;
            }
            memo.cps[o as usize] = memo.cep;
        }
        let mut live = [0usize; MAXL];
        let mut r = 0usize;
        let mut ng = 0usize;
        for i in 0..d {
            let left = (gr.gc[i] - (code >> gr.sh[i] & gr.mk[i])) as u32;
            if left == 0 { continue; }
            h += left * lbi(i);
            r += left as usize;
            live[ng] = i;
            ng += 1;
        }
        let mut add = 0u32;
        if use_hdyn() {
            for q in 0..r {
                let qq = q.min(n.min(14));
                let mut m = u32::MAX;
                for g in 0..ng { m = m.min(memo.cpc[cb + live[g] * CW + qq]); }
                if m == 0 { break; }
                add += m;
            }
        }
        let mut h_dyn = h + add;
        if hb {
            // 同じ底の色 c（家が開いていない）のレーンのうち「残す」ができるのは 1 本だけ（残すと c の家が開く）。
            // 動く数の和は「全部出す数の和 − 色ごとに一番大きい節約（全部 − 残す）」以上。
            // 余分（家の開いていない色の玉）は、動く数と別に、各順番 q で余分の下限が最小のレーンを足す。
            let mut s2 = 0u32;
            let mut sav = [0u32; 16];
            for g in 0..ng {
                let i = live[g];
                let left = (gr.gc[i] - (code >> gr.sh[i] & gr.mk[i])) as u32;
                s2 += left * its[i].3 as u32;
                if its[i].5 && o >> its[i].0 & 1 == 0 { let c0 = its[i].0 as usize; sav[c0] = sav[c0].max((its[i].3 - its[i].1) as u32); }
            }
            for c in 1..=n { s2 -= sav[c]; }
            let mut add2 = 0u32;
            for q in 0..r {
                let qq = q.min(n.min(14));
                let mut m = u32::MAX;
                for g in 0..ng { m = m.min(memo.cpc[cb + live[g] * CW + 18 + qq]); }
                if m == 0 { break; }
                add2 += m;
            }
            h_dyn = h_dyn.max(s2 + add2);
        }
        let mut best = h_dyn.max(stored);
        if best > budget {
            memo.stamp[key] = memo.ep << 8 | best.min(254);
            return best;
        }
        best = u32::MAX;
        // 手の候補を集め、(費用 + 残りの静的な下限, 足す色が残りに多い順) で並べてから試す。予算内の道が早く見つかる。
        let mut opts: [(u32, u32, usize, u32); 192] = [(0, 0, 0, 0); 192];
        let mut no = 0usize;
        // 残りのレーン全部の「全部の表」の色ごとの個数と、色ごとの「残す」ができるレーンの本数。手ごとにはそのレーンの分を 1 本引くだけにする。
        let mut call = [0u32; 16];
        let mut kc = [0u32; 16];
        for j in 0..d {
            let left = (gr.gc[j] - (code >> gr.sh[j] & gr.mk[j])) as u32;
            if left > 0 {
                for k in 1..=n { call[k] += left * its[j].4[k] as u32; }
                if its[j].5 { kc[its[j].0 as usize] += left; }
            }
        }
        for i in 0..d {
            if code >> gr.sh[i] & gr.mk[i] == gr.gc[i] { continue; }
            let (c0, nk, ref keep, nf, ref full, cankeep) = its[i];
            let nm = code + gr.gm[i];
            let hn = h - lbi(i);
            let _ = (nk, keep, nf);
            if cankeep && o >> c0 & 1 == 0 {
                let c = memo.cpc[cb + i * CW + 17];
                if c + hn > budget { best = best.min(c + hn); } else if no < 192 { opts[no] = (c + hn, 0, nm, (o | 1 << c0) | (c << 24)); no += 1; }
            }
            let fc = memo.cpc[cb + i * CW + 16];
            if fc + hn > budget { best = best.min(fc + hn); continue; }
            let mut present = 0u32;
            let mut kmask = 0u32;
            let mut cnt = [0u32; 16];
            for k in 1..=n {
                cnt[k] = call[k] - full[k] as u32;
                if cnt[k] > 0 { present |= 1 << k; }
                if kc[k] > (cankeep && k == c0 as usize) as u32 { kmask |= 1 << k; }
            }
            let mut rest = colors & !o & present;
            let mut add_none = true;
            if use_dom() {
                // 支配される候補を外す（最小値は変わらない。LOWER_BOUND_PROOF.md の「支配される手」）。
                // kmask: このあと残るレーンのうち「残す」ができるものの底の色。これに入らない色 c は家に足しても損をしない
                // （V(R, o∪{c}) ≤ V(R, o)）。
                let kp = cankeep && o >> c0 & 1 == 0;
                // (a) 「残す」ができるとき、「全部出して c0 を足す」は同じ o に高い費用で着くので外す。
                if kp { rest &= !(1u32 << c0); }
                // (b) 対称な色の組の中では、足す色は 1 色だけ試せばよい（詰め直すと同じ局面になる）。
                for &m in gr.cls {
                    let av = rest & m;
                    if av != 0 { rest = (rest & !m) | (av & av.wrapping_neg()); }
                }
                // (c) 何も足さない手は、kmask に入らない色を足す手か、（c0 が kmask に入らないなら）「残す」に負ける。
                if rest & !kmask != 0 || (kp && kmask >> c0 & 1 == 0) { add_none = false; }
            }
            if add_none && no < 192 { opts[no] = (fc + hn, 255, nm, o | (fc << 24)); no += 1; }
            while rest != 0 {
                let c = rest.trailing_zeros();
                rest &= rest - 1;
                if no < 192 { opts[no] = (fc + hn, 255 - cnt[c as usize].min(254), nm, (o | 1 << c) | (fc << 24)); no += 1; }
            }
        }
        assert!(no < 192);
        let ops = &mut opts[..no];
        let mut ord = [0u64; 192];
        for (t, x) in ops.iter().enumerate() { ord[t] = if use_order() { (x.0 as u64) << 40 | (x.1 as u64) << 32 } else { 0 } | t as u64; }
        if use_order() { ord[..no].sort_unstable(); }
        for &w in &ord[..no] {
            let (_, _, nm, oc) = ops[(w & 0xffff_ffff) as usize];
            let (c, no2) = (oc >> 24, oc & 0xff_ffff);
            let v = c + go(nm, no2, budget - c, n, colors, gr, memo);
            if v <= budget { return v; }
            best = best.min(v);
        }
        let best = best.max(stored);
        memo.stamp[key] = memo.ep << 8 | best.min(254);
        if tslot != usize::MAX {
            let tk = mk_key(gr);
            let ent = &mut memo.tt[tslot];
            if ent.0 == tfp && ent.1 == tk { ent.2 = ent.2.max(best.min(254) as u8); } else { *ent = (tfp, tk, best.min(254) as u8); }
        }
        best
    }
    thread_local! { static JTBUF: std::cell::RefCell<(Vec<u32>, Vec<u8>, u32)> = std::cell::RefCell::new((Vec::new(), Vec::new(), 0)); }
    thread_local! { static JTT: std::cell::RefCell<(Vec<(u64, [u64; 14], u8)>, Vec<u32>, Vec<u32>, u32)> = std::cell::RefCell::new((Vec::new(), Vec::new(), Vec::new(), 0)); }
    let ttmin = tt_min();
    JTT.with(|ttcell| JTBUF.with(|cell| {
        let mut ttall = ttcell.borrow_mut();
        let (ttg, cps, cpc, cep) = &mut *ttall;
        if ttmin > 0 && ttg.is_empty() { *ttg = vec![(0u64, [0u64; 14], 0u8); 1usize << tt_bits()]; }
        let ttmask = ttg.len().wrapping_sub(1);
        let mut guard = cell.borrow_mut();
        let (stamp, val, epoch) = &mut *guard;
        let mut best = u32::MAX;
        let nu = uni.len();
        for kset in 0usize..(1 << nu) {
            let mut k0 = 0u32;
            let mut ok = true;
            let mut its = items.clone();
            for (i, &(u, len)) in uni.iter().enumerate() {
                if kset >> i & 1 == 1 {
                    if k0 >> u & 1 == 1 { ok = false; break; }
                    k0 |= 1 << u;
                } else {
                    let mut full = [0u8; 16];
                    full[u as usize] = len;
                    its.push((u, 0, [0u8; 16], len, full, false));
                }
            }
            if !ok { continue; }
            its.sort_unstable();
            let mut gi: Vec<It> = Vec::new();
            let mut gc: Vec<usize> = Vec::new();
            for it in its { if gi.last() == Some(&it) { *gc.last_mut().unwrap() += 1; } else { gi.push(it); gc.push(1); } }
            let (mut gm, mut sh, mut mk) = (Vec::new(), Vec::new(), Vec::new());
            let mut bits = 0u32;
            let mut done = 0usize;
            for &c in &gc {
                let w = usize::BITS - c.leading_zeros();
                gm.push(1usize << bits); sh.push(bits); mk.push((1usize << w) - 1);
                done |= c << bits;
                bits += w;
            }
            let tot = done + 1;
            let its = gi;
            let size = tot << (n + 1);
            if stamp.len() < size { stamp.resize(size, 0); }
            let _ = &val;
            *epoch += 1;
            if *epoch >= 1 << 24 { stamp.iter_mut().for_each(|x| *x = 0); *epoch = 1; }
            let no = 1usize << (n + 1);
            if cps.len() < no { cps.resize(no, 0); }
            if cpc.len() < no * its.len() * 34 { cpc.resize(no * its.len() * 34, 0); }
            *cep = cep.wrapping_add(1);
            if *cep == 0 { cps.iter_mut().for_each(|x| *x = 0); *cep = 1; }
            let mut memo = Memo { stamp: &mut *stamp, val: &mut *val, ep: *epoch, tt: &mut *ttg, ttmask, ttmin, nodes: 0, cps: &mut *cps, cpc: &mut *cpc, cep: *cep };
            let pm: Vec<u32> = its.iter().map(|it| { let mut m = 0u32; for k in 1..=n { if it.4[k] > 0 { m |= 1 << k; } } m }).collect();
            // 空レーンの家: k0 に無い色から 0〜e 色（どのレーンにも出てこない色は足しても同じなので外す）
            let free = colors & !k0 & pm.iter().fold(0, |a, &m| a | m);
            // 色の組（2 色以上の組だけ）: 底の色でなく、全部のレーンの残す表・全部の表で個数が同じ色
            let mut cls: Vec<u32> = Vec::new();
            if use_sym() {
                let mut c0s = 0u32;
                for it in &its { c0s |= 1 << it.0; }
                let mut used = c0s | 1;
                for k in 1..=n {
                    if used >> k & 1 == 1 { continue; }
                    let mut m = 1u32 << k;
                    for k2 in k + 1..=n {
                        if used >> k2 & 1 == 1 { continue; }
                        if its.iter().all(|it| it.2[k] == it.2[k2] && it.4[k] == it.4[k2]) { m |= 1 << k2; }
                    }
                    used |= m;
                    if m.count_ones() > 1 { cls.push(m); }
                }
            }
            // レーンの形の符号（底の色・底の同色の数・残せるか・残す表の各色の個数）。形が同じなら同じ値、違えば違う値。
            let sg: Vec<u64> = its.iter().map(|it| {
                let mut x = (it.0 as u64) << 60 | ((it.3 - it.1) as u64) << 56 | (it.5 as u64) << 55;
                for k in 1..=n { x |= (it.2[k] as u64) << (4 * (k - 1)); }
                x
            }).collect();
            let zg: Vec<u64> = sg.iter().map(|&x| { let mut z = x.wrapping_mul(0xC2B2_AE3D_27D4_EB4F); z ^= z >> 29; z.wrapping_mul(0x1656_67B1_9E37_79F9) | 1 }).collect();
            let mut ord: Vec<usize> = (0..its.len()).collect();
            ord.sort_unstable_by_key(|&i| sg[i]);
            let gr = Gr { its: &its, gc: &gc, gm: &gm, sh: &sh, mk: &mk, pm: &pm, done, cls: &cls, sg: &sg, zg: &zg, ord: &ord };
            // 空レーンの家の選び方も、補題（9 番）で負ける選び方を外す: e 色に満たず、まだ足せる色のうち K に入らないものがあるなら、
            // それを足した選び方に負けない。
            let kall = its.iter().filter(|it| it.5).fold(0u32, |a, it| a | 1 << it.0);
            let mut sub = free;
            loop {
                let dominated = use_dom() && (sub.count_ones() as usize) < e && free & !sub & !kall != 0;
                if (sub.count_ones() as usize) <= e && !dominated {
                    let v = go(0, k0 | sub, thr, n, colors, &gr, &mut memo);
                    JNODES.fetch_add(std::mem::take(&mut memo.nodes), std::sync::atomic::Ordering::Relaxed);
                    if v <= thr { return v; }
                    if v < best { best = v; }
                }
                if sub == 0 { break; }
                sub = (sub - 1) & free;
            }
        }
        best
    }))
}
/// 呼び出しをまたぐ表を使う最小の残りレーン数（0 で使わない）。COLORLB_TT=k
fn tt_min() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("COLORLB_TT").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}
/// 表の大きさ（2^bits 項目、1 項目 120 バイト、スレッドごと）。COLORLB_TTBITS
fn tt_bits() -> u32 {
    static V: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("COLORLB_TTBITS").ok().and_then(|v| v.parse().ok()).unwrap_or(20))
}
/// 以前の J（レーンを 1 本ずつ区別する版）。jtime で新しい版と値を突き合わせる用。
#[allow(dead_code)]
fn jlb_ref(s: &State, n: usize) -> u32 {
    // (底の色, 残すときの動く数, 残すときの表, 全部出すときの数, 全部の表, 残せるか)
    type It = (u8, u8, [u8; 16], u8, [u8; 16], bool);
    let mut e = 0usize;
    let mut items: Vec<It> = Vec::new();
    let mut uni: Vec<(u8, u8)> = Vec::new();
    for &l in &s[..=n] {
        let ht = height(l);
        if ht == 0 { e += 1; continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        if r == ht { uni.push((c0, ht as u8)); continue; }
        let mut keep = [0u8; 16];
        for k in r..ht { keep[ball(l, k) as usize] += 1; }
        let mut full = keep;
        full[c0 as usize] += r as u8;
        items.push((c0, (ht - r) as u8, keep, ht as u8, full, true));
    }
    let colors: u32 = ((1u32 << (n + 1)) - 1) & !1;
    struct Memo<'a> { stamp: &'a mut Vec<u32>, val: &'a mut Vec<u8>, ep: u32 }
    fn cost(cnt: &[u8; 16], o: u32, n: usize) -> u32 {
        let mut c = 0u32;
        for k in 1..=n { if o >> k & 1 == 0 { c += cnt[k] as u32; } }
        c
    }
    // pm[i]: レーン i の全部の表に出てくる色。残りのレーンに出てこない色を家に足しても、費用も「残せるか」も変わらない
    // （底の色 c0 のレーンが残っていれば c0 は表に出る）ので、足す色の候補から外す（最小値は変わらない）。
    fn go(mask: usize, o: u32, n: usize, colors: u32, its: &[It], pm: &[u32], memo: &mut Memo) -> u32 {
        let d = its.len();
        if mask == (1 << d) - 1 { return 0; }
        let key = (mask << (n + 1)) | o as usize;
        if memo.stamp[key] == memo.ep { return memo.val[key] as u32; }
        let mut best = u32::MAX;
        for i in 0..d {
            if mask >> i & 1 == 1 { continue; }
            let (c0, nk, ref keep, nf, ref full, cankeep) = its[i];
            let nm = mask | 1 << i;
            if cankeep && o >> c0 & 1 == 0 {
                let v = nk as u32 + cost(keep, o, n) + go(nm, o | 1 << c0, n, colors, its, pm, memo);
                if v < best { best = v; }
            }
            let fc = nf as u32 + cost(full, o, n);
            if fc >= best { continue; }
            let v = fc + go(nm, o, n, colors, its, pm, memo);
            if v < best { best = v; }
            let mut present = 0u32;
            for j in 0..d { if nm >> j & 1 == 0 { present |= pm[j]; } }
            let mut rest = colors & !o & present;
            while rest != 0 {
                let c = rest.trailing_zeros();
                rest &= rest - 1;
                let v = fc + go(nm, o | 1 << c, n, colors, its, pm, memo);
                if v < best { best = v; }
            }
        }
        memo.stamp[key] = memo.ep;
        memo.val[key] = best.min(254) as u8;
        best
    }
    thread_local! { static JBUF0: std::cell::RefCell<(Vec<u32>, Vec<u8>, u32)> = std::cell::RefCell::new((Vec::new(), Vec::new(), 0)); }
    JBUF0.with(|cell| {
        let mut guard = cell.borrow_mut();
        let (stamp, val, epoch) = &mut *guard;
        let mut best = u32::MAX;
        let nu = uni.len();
        for kset in 0usize..(1 << nu) {
            let mut k0 = 0u32;
            let mut ok = true;
            let mut its = items.clone();
            for (i, &(u, len)) in uni.iter().enumerate() {
                if kset >> i & 1 == 1 {
                    if k0 >> u & 1 == 1 { ok = false; break; }
                    k0 |= 1 << u;
                } else {
                    let mut full = [0u8; 16];
                    full[u as usize] = len;
                    its.push((u, 0, [0u8; 16], len, full, false));
                }
            }
            if !ok { continue; }
            let size = (1usize << its.len()) << (n + 1);
            if stamp.len() < size { stamp.resize(size, 0); val.resize(size, 0); }
            *epoch = epoch.wrapping_add(1);
            if *epoch == 0 { stamp.iter_mut().for_each(|x| *x = 0); *epoch = 1; }
            let mut memo = Memo { stamp: &mut *stamp, val: &mut *val, ep: *epoch };
            let pm: Vec<u32> = its.iter().map(|it| { let mut m = 0u32; for k in 1..=n { if it.4[k] > 0 { m |= 1 << k; } } m }).collect();
            // 空レーンの家: k0 に無い色から 0〜e 色（どのレーンにも出てこない色は足しても同じなので外す）
            let free = colors & !k0 & pm.iter().fold(0, |a, &m| a | m);
            let mut sub = free;
            loop {
                if (sub.count_ones() as usize) <= e {
                    let v = go(0, k0 | sub, n, colors, &its, &pm, &mut memo);
                    if v < best { best = v; }
                }
                if sub == 0 { break; }
                sub = (sub - 1) & free;
            }
        }
        best
    })
}
// Assignment keeps lane order because top-to-bottom order affects pen.
// The suffix memo instead uses the shape and colour key from J1Problem.
#[derive(Default)]
struct J1Hasher(u64);
impl std::hash::Hasher for J1Hasher {
    fn finish(&self) -> u64 { self.0 }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes { self.write_u64(b as u64); }
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x517cc1b727220a95);
    }
    fn write_usize(&mut self, value: usize) { self.write_u64(value as u64); }
    fn write_u32(&mut self, value: u32) { self.write_u64(value as u64); }
    fn write_u16(&mut self, value: u16) { self.write_u64(value as u64); }
    fn write_u8(&mut self, value: u8) { self.write_u64(value as u64); }
}
type J1Map<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<J1Hasher>>;

// Full keys are compared even when their hashes collide. Colour labels stay
// fixed, except for the proven interchangeable colour classes in open_sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct J1SharedKey {
    shapes: [u64; MAXL],
    o: u16,
    n: u8,
}
const J1_SHARED_LIMIT: usize = 1 << 16;

fn j1_share_min() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("COLORLB_J1SHARE_MIN").ok().and_then(|v| v.parse().ok()).unwrap_or(9))
}

#[derive(Default)]
struct J1Memo {
    stamp: Vec<u32>,
    lo: Vec<u16>,
    hi: Vec<u16>,
    epoch: u32,
    shared: J1Map<J1SharedKey, (u32, u32)>,
}
impl J1Memo {
    fn reset(&mut self, size: usize) {
        if self.stamp.len() < size {
            self.stamp.resize(size, 0);
            self.lo.resize(size, 0);
            self.hi.resize(size, u16::MAX);
        }
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 { self.stamp.fill(0); self.epoch = 1; }
    }
    fn bounds(&self, key: usize) -> (u32, u32) {
        if self.stamp[key] == self.epoch {
            (self.lo[key] as u32, self.hi[key] as u32)
        } else { (0, u32::MAX) }
    }
    fn record(&mut self, key: usize, budget: u32, pass: bool) {
        if self.stamp[key] != self.epoch {
            self.stamp[key] = self.epoch;
            self.lo[key] = 0;
            self.hi[key] = u16::MAX;
        }
        if pass { self.hi[key] = self.hi[key].min(budget as u16); }
        else { self.lo[key] = self.lo[key].max((budget + 1) as u16); }
    }
    fn record_shared(&mut self, key: J1SharedKey, value: u32, pass: bool) {
        // Clearing before insertion keeps capacity bounded, too (about
        // 17 MiB on 64-bit targets at the next HashMap capacity boundary).
        if self.shared.len() >= J1_SHARED_LIMIT { self.shared.clear(); }
        let v = self.shared.entry(key).or_insert((0, u32::MAX));
        if pass { v.1 = v.1.min(value); } else { v.0 = v.0.max(value); }
    }
}
struct J1Lane {
    lane: Lane,
    c0: u32,
    run: usize,
    dirty: bool,
    counts: [u8; 16],
    // [full, keep], indexed by the open colour set (colour c uses bit c-1).
    costs: Vec<[u8; 2]>,
    // [open set][prior openings]: bounds above static movement, and above
    // actual movement, respectively. The latter permits grouping bottom runs.
    relax: Vec<std::cell::OnceCell<[[u8; 2]; MAXL]>>,
    shape: u64,
    count_order: [u8; 16],
}
impl J1Lane {
    fn new(lane: Lane, n: usize) -> Self {
        let ht = height(lane);
        let c0 = ball(lane, 0);
        let mut run = 1;
        while run < ht && ball(lane, run) == c0 { run += 1; }
        let dirty = run != ht;
        let mut counts = [0u8; 16];
        for p in 0..ht { counts[ball(lane, p) as usize - 1] += 1; }
        let mut costs = vec![[0; 2]; 1 << n];
        costs[0] = [(2 * ht) as u8, (2 * (ht - run)) as u8];
        for o in 1usize..1 << n {
            let c = o.trailing_zeros() as usize;
            let prev = costs[o & (o - 1)];
            costs[o] = [prev[0] - counts[c], prev[1] - (counts[c] - if c == c0 as usize - 1 { run as u8 } else { 0 })];
        }
        let relax = (0..1usize << n).map(|_| std::cell::OnceCell::new()).collect();
        let mut count_order = std::array::from_fn(|c| c as u8);
        count_order.sort_unstable_by_key(|&c| std::cmp::Reverse(counts[c as usize]));
        let mut shape = (c0 as u64 - 1) | ((run as u64) << 4) | ((dirty as u64) << 8);
        for c in 0..n { shape |= (counts[c] as u64) << (9 + 4 * c); }
        Self { lane, c0: 1 << (c0 - 1), run, dirty, counts, costs, relax, shape, count_order }
    }

    fn relax(&self, o: usize) -> &[[u8; 2]; MAXL] {
        self.relax[o].get_or_init(|| {
            let ht = height(self.lane);
            let c0 = self.c0.trailing_zeros() as usize;
            let cankeep = self.dirty && o & self.c0 as usize == 0;
            let movement = ht - if cankeep { self.run } else { 0 };
            let mut full = [0u8; 16];
            let mut keep = [0u8; 16];
            let (mut nf, mut nk) = (0, 0);
            for &c in &self.count_order {
                let c = c as usize;
                if self.counts[c] == 0 { break; }
                if o >> c & 1 == 0 {
                    full[nf] = self.counts[c]; nf += 1;
                    if c != c0 { keep[nk] = self.counts[c]; nk += 1; }
                }
            }
            let (mut sf, mut sk) = (0, 0);
            let mut relax = [[0; 2]; MAXL];
            for q in 0..MAXL {
                let fc = self.costs[o][0] - sf;
                let kc = self.costs[o][1] - sk;
                relax[q] = [(if cankeep { fc.min(kc) } else { fc }) - movement as u8,
                    if cankeep { (fc - ht as u8).min(kc - (ht - self.run) as u8) } else { fc - ht as u8 }];
                sf += full[q]; sk += keep[q];
            }
            relax
        })
    }
}
struct J1Problem {
    n: usize,
    lanes: Vec<J1Lane>,
    full: usize,
    size: usize,
    groups: Vec<usize>,
    shape_masks: Vec<usize>,
    open_sets: Vec<u32>,
    classes: Vec<u32>,
    share_min: usize,
    share_enabled: bool,
    remaining: Vec<([u8; 16], [u8; 16])>,
}
impl J1Problem {
    fn new(n: usize, lanes: Vec<J1Lane>) -> Self {
        let full = (1usize << lanes.len()) - 1;
        let mut groups: Vec<usize> = Vec::new();
        for (i, l) in lanes.iter().enumerate() {
            if let Some(g) = groups.iter_mut().find(|g| {
                let other = &lanes[g.trailing_zeros() as usize];
                (l.c0, l.run, l.dirty, l.counts) == (other.c0, other.run, other.dirty, other.counts)
            }) { *g |= 1 << i; }
            else { groups.push(1 << i); }
        }
        // In each shape group, put the cleared lanes in the lowest positions.
        // Assignment still uses the original mask and the original lane order.
        let shape_masks = (0..=full).map(|mask| {
            let mut canonical = 0;
            for &g in &groups {
                let mut bits = g;
                for _ in 0..(mask & g).count_ones() {
                    canonical |= bits & bits.wrapping_neg();
                    bits &= bits - 1;
                }
            }
            canonical
        }).collect();
        let mut used = lanes.iter().fold(0, |m, l| m | l.c0);
        let mut classes = Vec::new();
        for c in 0..n {
            if used >> c & 1 != 0 { continue; }
            let mut m = 1u32 << c;
            for k in c + 1..n {
                if used >> k & 1 == 0 && lanes.iter().all(|l| l.counts[c] == l.counts[k]) {
                    m |= 1 << k;
                }
            }
            used |= m;
            if m.count_ones() > 1 { classes.push(m); }
        }
        let open_sets = (0..1u32 << n).map(|mut o| {
            for &m in &classes {
                let mut bits = m;
                let mut low = 0;
                for _ in 0..(o & m).count_ones() {
                    low |= bits & bits.wrapping_neg();
                    bits &= bits - 1;
                }
                o = (o & !m) | low;
            }
            o
        }).collect();
        let mut remaining = vec![([0u8; 16], [0u8; 16]); full + 1];
        for mask in (0..full).rev() {
            let i = (full ^ mask).trailing_zeros() as usize;
            let l = &lanes[i];
            let (mut counts, mut keeps) = remaining[mask | 1 << i];
            for c in 0..n { counts[c] += l.counts[c]; }
            if l.dirty { keeps[l.c0.trailing_zeros() as usize] += 1; }
            remaining[mask] = (counts, keeps);
        }
        let share_min = j1_share_min();
        let share_enabled = lanes.len() >= share_min;
        Self { n, lanes, full, size: (full + 1) << n, groups, shape_masks, open_sets, classes, share_min, share_enabled, remaining }
    }

    fn suffix_key(&self, mask: usize, o: u32) -> usize {
        (self.shape_masks[mask] << self.n) | self.open_sets[o as usize] as usize
    }

    fn shared_key(&self, mask: usize, o: u32) -> J1SharedKey {
        let mut shapes = [0; MAXL];
        let mut len = 0;
        for (i, l) in self.lanes.iter().enumerate() {
            if mask >> i & 1 == 0 { shapes[len] = l.shape; len += 1; }
        }
        shapes[..len].sort_unstable();
        J1SharedKey { shapes, o: self.open_sets[o as usize] as u16, n: self.n as u8 }
    }

    // Lower bounds allow at most q new colours before position q. Keeping a
    // bottom run can save it only once per colour, even across duplicate lanes.
    fn lower(&self, mask: usize, o: u32) -> (u32, u32) {
        let mut movement = 0;
        let mut full = 0;
        let mut savings = [0u32; 16];
        let mut rows = [u32::MAX; MAXL];
        let mut extras = [u32::MAX; MAXL];
        let left = (self.full ^ mask).count_ones() as usize;
        if left == 0 { return (0, 0); }
        for (i, l) in self.lanes.iter().enumerate() {
            if mask >> i & 1 != 0 { continue; }
            let h = height(l.lane) as u32;
            let keep = l.dirty && o & l.c0 == 0;
            let m = h - if keep { l.run as u32 } else { 0 };
            movement += m;
            full += h;
            if keep {
                let c = l.c0.trailing_zeros() as usize;
                savings[c] = savings[c].max(l.run as u32);
            }
            let relax = l.relax(o as usize);
            for q in 0..left {
                let r = relax[q];
                rows[q] = rows[q].min(r[0] as u32);
                extras[q] = extras[q].min(r[1] as u32);
            }
        }
        ((movement + rows[..left].iter().sum::<u32>())
            .max(full - savings.iter().sum::<u32>() + extras[..left].iter().sum::<u32>()), movement)
    }

    // Bounds belong to V(R, o), never to J's cache that minimizes over O0.
    fn go(&self, mask: usize, o: u32, budget: u32, memo: &mut J1Memo) -> bool {
        // Consult the cross-call table only at J1's V entry points, leaving
        // recursive nodes on the inexpensive, per-problem dense memo.
        if !self.share_enabled || ((self.full ^ mask).count_ones() as usize) < self.share_min {
            return self.go_bound(mask, o, budget, memo) <= budget;
        }
        let local = self.suffix_key(mask, o);
        let (lo, hi) = memo.bounds(local);
        if hi <= budget { return true; }
        if lo > budget { return false; }
        let key = self.shared_key(mask, o);
        if let Some(&(lo, hi)) = memo.shared.get(&key) {
            if hi < u16::MAX as u32 { memo.record(local, hi, true); }
            if lo > 0 { memo.record(local, lo - 1, false); }
            if hi <= budget { return true; }
            if lo > budget { return false; }
        }
        let value = self.go_bound(mask, o, budget, memo);
        let pass = value <= budget;
        if j1_cap() == 0 || J1CNT.with(|c| c.get()) <= j1_cap() {
            memo.record_shared(key, value, pass);
        }
        pass
    }

    // A passing value is a witnessed cost; a failing value is a lower bound.
    fn go_bound(&self, mask: usize, o: u32, budget: u32, memo: &mut J1Memo) -> u32 {
        if mask == self.full { return 0; }
        // Aborted calls are accepted conservatively by j1_le. Do not memoize
        // their return as a witness (nor propagate it into an ancestor memo).
        if j1_tick() { return 0; }
        let mask = self.shape_masks[mask];
        let o = self.open_sets[o as usize];
        let key = (mask << self.n) | o as usize;
        let (lo, hi) = memo.bounds(key);
        if hi <= budget { return hi; }
        if lo > budget { return lo; }
        let (lower, movement) = self.lower(mask, o);
        if lower > budget {
            memo.record(key, lower - 1, false); return lower;
        }
        let (call, kc) = &self.remaining[mask];
        // One integer per option: estimate, opening preference, cost, mask, o.
        let mut opts = [0u64; MAXL * 14];
        let mut count = 0;
        let mut best = u32::MAX;
        for &g in &self.groups {
            let remaining = g & !mask;
            if remaining == 0 { continue; }
            let i = remaining.trailing_zeros() as usize;
            let l = &self.lanes[i];
            let nm = mask | 1 << i;
            let keep = l.dirty && o & l.c0 == 0;
            let hn = movement - (height(l.lane) - if keep { l.run } else { 0 }) as u32;
            let mut push = |cost: u32, no: u32, preference: u32| {
                let estimate = cost + hn;
                if estimate > budget { best = best.min(estimate); }
                else {
                    opts[count] = (estimate as u64) << 48 | (preference as u64) << 40
                        | (cost as u64) << 32 | (nm as u64) << 16 | no as u64;
                    count += 1;
                }
            };
            if keep { push(l.costs[o as usize][1] as u32, o | l.c0, 0); }
            let cost = l.costs[o as usize][0] as u32;
            if cost + hn > budget { best = best.min(cost + hn); continue; }
            let mut rest = 0u32;
            let mut kmask = 0u32;
            let mut cnt = [0u32; 16];
            for c in 0..self.n {
                cnt[c] = (call[c] - l.counts[c]) as u32;
                if cnt[c] > 0 { rest |= 1 << c; }
                if kc[c] > (l.dirty && l.c0 == 1 << c) as u8 { kmask |= 1 << c; }
            }
            rest &= !o;
            // J's proven dominance applies only inside this suffix search.
            if keep { rest &= !l.c0; }
            for &m in &self.classes {
                let available = rest & m;
                if available != 0 { rest = (rest & !m) | (available & available.wrapping_neg()); }
            }
            if rest & !kmask == 0 && !(keep && l.c0 & kmask == 0) {
                push(cost, o, 255);
            }
            while rest != 0 {
                let bit = rest & rest.wrapping_neg();
                rest &= rest - 1;
                push(cost, o | bit, 255 - cnt[bit.trailing_zeros() as usize]);
            }
        }
        opts[..count].sort_unstable();
        for &opt in &opts[..count] {
            let cost = ((opt >> 32) & 255) as u32;
            let nm = ((opt >> 16) & 65535) as usize;
            let no = (opt & 65535) as u32;
            let value = cost + self.go_bound(nm, no, budget - cost, memo);
            if value <= budget {
                if j1_cap() == 0 || J1CNT.with(|c| c.get()) <= j1_cap() {
                    memo.record(key, value, true);
                }
                return value;
            }
            best = best.min(value);
        }
        best = best.max(lo);
        memo.record(key, best - 1, false);
        best
    }
}

#[derive(Default)]
struct J1SubsetMemo {
    dense: J1Memo,
    sparse: J1Map<usize, (u32, u32)>,
    is_dense: bool,
}
impl J1SubsetMemo {
    fn reset(&mut self, size: usize) {
        // Bound scratch memory for larger N; dense epochs avoid clearing the
        // usual N<=7 tables between first actions and successive puzzle states.
        self.is_dense = size <= 1 << 22;
        if self.is_dense { self.dense.reset(size); }
        self.sparse.clear();
    }
    fn bounds(&self, key: usize) -> (u32, u32) {
        if self.is_dense { self.dense.bounds(key) }
        else { self.sparse.get(&key).copied().unwrap_or((0, u32::MAX)) }
    }
    fn record(&mut self, key: usize, budget: u32, pass: bool) {
        if self.is_dense { self.dense.record(key, budget, pass); }
        else {
            let v = self.sparse.entry(key).or_insert((0, u32::MAX));
            if pass { v.1 = v.1.min(budget); } else { v.0 = v.0.max(budget + 1); }
        }
    }
}
struct J1Assignment<'a> {
    problem: &'a J1Problem,
    // Bit positions retain top-down order for the first-use home penalty.
    temps: &'a [u8],
    end: Vec<u32>,
    closed: Vec<usize>,
    o0: u32,
    base: &'a mut J1Memo,
    memo: &'a mut J1SubsetMemo,
}
impl J1Assignment<'_> {
    fn end_cost(temps: &[u8], below: &[[u8; 16]], homes: &[(u8, usize)], buffer: usize) -> Vec<u32> {
        let size = 1usize << temps.len();
        let mut end: Vec<u32> = (0..size).map(|r| (r.count_ones() as usize).saturating_sub(buffer) as u32).collect();
        for &(c, cap) in homes {
            let old = end.clone();
            for (r, value) in end.iter_mut().enumerate().skip(1) {
                let mut sub = r;
                while sub != 0 {
                    if sub.count_ones() as usize <= cap {
                        // First temp on this home is the highest selected one.
                        *value = (*value).min(old[r ^ sub] + below[sub.trailing_zeros() as usize][c as usize] as u32);
                    }
                    sub = (sub - 1) & r;
                }
            }
        }
        end
    }

    // sl: penalty still affordable. Every J plan costs at least J(s), so a plan
    // whose total penalty exceeds thr - J(s) fails anyway; pruning it keeps
    // every recorded bound true.
    fn go(&mut self, mask: usize, o: u32, r: usize, budget: u32, sl: u32) -> bool {
        if mask == self.problem.full { return self.end[r] <= budget; }
        if j1_tick() { return true; }
        let key = (((mask << self.problem.n) | o as usize) << self.temps.len()) | r;
        let (lo, hi) = self.memo.bounds(key);
        if hi <= budget { return true; }
        if lo > budget { return false; }
        if !self.problem.go(mask, o, budget, self.base) {
            self.memo.record(key, budget, false); return false;
        }
        let base_key = self.problem.suffix_key(mask, o);
        // A witnessed J suffix plus a complete end assignment is a J1 plan.
        if self.base.bounds(base_key).1.saturating_add(self.end[r]) <= budget { return true; }
        if self.end[r] <= sl && self.end[r] <= budget && self.problem.go(mask, o, budget - self.end[r], self.base) { return true; }
        let closed = r & self.closed[o as usize];
        for i in 0..self.problem.lanes.len() {
            if mask >> i & 1 != 0 { continue; }
            let l = &self.problem.lanes[i];
            if (0..i).any(|j| mask >> j & 1 == 0 && self.problem.lanes[j].lane == l.lane) { continue; }
            let h = height(l.lane);
            let mut displacement = [0u32; MAXL + 1];
            for count in 1..=self.temps.len() {
                let need = count.saturating_sub(self.problem.n - h);
                displacement[count] = if need == 0 { 0 } else {
                    displacement[count - 1] + ((o & !self.o0 & (1 << (ball(l.lane, h - need) - 1)) != 0) as u32)
                };
            }
            let nm = mask | 1 << i;
            // Try free placements first, then all other subsets. No dominance
            // discards temp positions: those positions still affect end[r].
            // A closed temp (colour not yet open) costs 1 here and uses space;
            // leaving it to the end costs at most 1 (free) and never more.
            let avail = r & !closed;
            // Temps in avail are open now and stay open, so later lanes and the
            // end see only their count, except homes, where a lower temp never
            // costs more. Hence placing the top k of avail is optimal for each k,
            // and k is worth trying only where the next temp would add displacement.
            let a = avail.count_ones() as usize;
            let mut sub = 0usize;
            let mut left = avail;
            for k in 0..=a {
                if k == a || displacement[k] < displacement[k + 1] {
                    let pen = displacement[k];
                    if pen > sl { break; }
                    let cost = l.costs[o as usize][1] as u32 + pen;
                    if l.dirty && o & l.c0 == 0 && cost <= budget && self.go(nm, o | l.c0, r ^ sub, budget - cost, sl - pen) {
                        self.memo.record(key, budget, true); return true;
                    }
                    let cost = l.costs[o as usize][0] as u32 + pen;
                    if cost <= budget {
                        let mut rest = ((1u32 << self.problem.n) - 1) & !o;
                        loop {
                            let bit = rest & rest.wrapping_neg();
                            if self.go(nm, o | bit, r ^ sub, budget - cost, sl - pen) {
                                self.memo.record(key, budget, true); return true;
                            }
                            if rest == 0 { break; }
                            rest &= rest - 1;
                        }
                    }
                }
                if k < a { let b = left & left.wrapping_neg(); sub |= b; left ^= b; }
            }
        }
        self.memo.record(key, budget, false);
        false
    }
}

/// Exactly jfirst.J1 <= thr. No J dominance rules are applied to home choices
/// or the first action: those choices also determine the temporary slots.
// J1 の 1 回の計算の節点の上限（COLORLB_J1NODES、0 で無制限）。超えたら「落とせない」として通す（通すのは常に安全）。
thread_local! { static J1CNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
static J1ABORT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn j1_cap() -> u64 {
    static V: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("COLORLB_J1NODES").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}
#[inline]
fn j1_tick() -> bool {
    let cap = j1_cap();
    if cap == 0 { if j1_stat() { J1CNT.with(|c| c.set(c.get() + 1)); } return false; }
    J1CNT.with(|c| { let v = c.get() + 1; c.set(v); v > cap })
}
fn j1_sl() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("COLORLB_J1SL").is_ok())
}
fn j1_stat() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("COLORLB_J1STAT").is_ok())
}
fn j1_le(s: &State, n: usize, thr: u32) -> bool {
    J1CNT.with(|c| c.set(0));
    let t0 = if j1_stat() { Some(std::time::Instant::now()) } else { None };
    let r = j1_le_inner(s, n, thr);
    if let Some(t0) = t0 {
        let us = t0.elapsed().as_micros();
        eprintln!("J1STAT {} {} {} {} {} {}", thr, r as u8, J1CNT.with(|c| c.get()), us, jlb(s, n), show(s, n));
    }
    if j1_cap() > 0 && J1CNT.with(|c| c.get()) > j1_cap() { J1ABORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed); return true; }
    r
}
fn j1_le_inner(s: &State, n: usize, thr: u32) -> bool {
    // Moving every ball in full plans costs at most 2*n*(n+1), and free
    // placements add at most n. Keep budgets small for the u16 memo bounds.
    if thr >= (2 * n * (n + 1) + n) as u32 { return true; }
    // Slack 0 (J = thr) is the common rejecting case. Asking J <= thr-1 costs
    // more than the pruning saves (9/117: 647s vs 628s), so it is opt-in.
    let sl = if j1_sl() && thr > 0 && !jle_cached(s, n, thr - 1) { 0 } else { u32::MAX };
    let mut lanes = Vec::new();
    let mut uni = Vec::new();
    let mut empties = 0;
    for &lane in &s[..=n] {
        let ht = height(lane);
        if ht == 0 { empties += 1; continue; }
        let l = J1Lane::new(lane, n);
        if !l.dirty { uni.push(lanes.len()); }
        lanes.push(l);
    }
    let full = (1 << lanes.len()) - 1;
    let problem = J1Problem::new(n, lanes);
    thread_local! {
        static J1BUF: std::cell::RefCell<(J1Memo, J1SubsetMemo)> = Default::default();
    }
    J1BUF.with(|cell| {
        let mut guard = cell.borrow_mut();
        let (base, memo) = &mut *guard;
        base.reset(problem.size);
        let colors = (1u32 << n) - 1;
        for kept in 0usize..1 << uni.len() {
            let mut mask = 0;
            let mut o_kept = 0;
            let mut homes_kept = Vec::new();
            let mut valid = true;
            for (u, &j) in uni.iter().enumerate() {
                if kept >> u & 1 == 0 { continue; }
                let l = &problem.lanes[j];
                if o_kept & l.c0 != 0 { valid = false; break; }
                mask |= 1 << j;
                o_kept |= l.c0;
                if height(l.lane) < n { homes_kept.push((ball(l.lane, 0) - 1, n - height(l.lane))); }
            }
            if !valid { continue; }
            if mask == full { return true; }
            let free = colors & !o_kept;
            let mut sub = free;
            loop {
                let k = sub.count_ones() as usize;
                if k <= empties {
                    let o0 = o_kept | sub;
                    if problem.go(mask, o0, thr, base) {
                        let buffer = n * (empties - k);
                        let mut homes = homes_kept.clone();
                        for c in 0..n { if sub >> c & 1 != 0 { homes.push((c as u8, n)); } }
                        for (first, l) in problem.lanes.iter().enumerate() {
                            if mask >> first & 1 != 0 { continue; }
                            if (0..first).any(|j| mask >> j & 1 == 0 && problem.lanes[j].lane == l.lane) { continue; }
                            for keep in [true, false] {
                                if keep && (!l.dirty || o0 & l.c0 != 0) { continue; }
                                let cost = l.costs[o0 as usize][keep as usize] as u32;
                                if cost > thr { continue; }
                                let budget = thr - cost;
                                let nm = mask | 1 << first;
                                // Build the temp tables only if some opening passes the J suffix.
                                let mut good = 0u64; // bit 0: open nothing / keep; bit c+1: open colour c
                                {
                                    let mut opens = if keep { l.c0 } else { colors & !o0 };
                                    loop {
                                        let bit = if opens == 0 { 0 } else { opens & opens.wrapping_neg() };
                                        if problem.go(nm, o0 | bit, budget, base) {
                                            good |= if bit == 0 || keep { 1 } else { 1 << (bit.trailing_zeros() + 1) };
                                        }
                                        if keep || opens == 0 { break; }
                                        opens &= opens - 1;
                                    }
                                }
                                if good == 0 { continue; }
                                let mut temps = Vec::new();
                                let mut below = Vec::new();
                                let mut counts = [0u8; 16];
                                for p in (if keep { l.run } else { 0 })..height(l.lane) {
                                    let c = ball(l.lane, p) - 1;
                                    if o0 >> c & 1 != 0 { counts[c as usize] += 1; }
                                    else { temps.push(c); below.push(counts); }
                                }
                                temps.reverse(); below.reverse();
                                let end = J1Assignment::end_cost(&temps, &below, &homes, buffer);
                                let mut closed = vec![0; 1 << n];
                                for (o, xs) in closed.iter_mut().enumerate() {
                                    for (i, &c) in temps.iter().enumerate() { if o >> c & 1 == 0 { *xs |= 1 << i; } }
                                }
                                memo.reset(problem.size << temps.len());
                                let mut assignment = J1Assignment { problem: &problem, temps: &temps, end, closed, o0, base, memo };
                                let mut opens = if keep { l.c0 } else { colors & !o0 };
                                // Zero is the 'full, open nothing' action.
                                loop {
                                    let bit = if opens == 0 { 0 } else { opens & opens.wrapping_neg() };
                                    let o = o0 | bit;
                                    let g = if bit == 0 || keep { 1 } else { 1 << (bit.trailing_zeros() + 1) };
                                    if good & g != 0 && assignment.go(nm, o, (1 << temps.len()) - 1, budget, sl) { return true; }
                                    if keep || opens == 0 { break; }
                                    opens &= opens - 1;
                                }
                            }
                        }
                    }
                }
                if sub == 0 { break; }
                sub = (sub - 1) & free;
            }
        }
        false
    })
}
fn j1_exact(s: &State, n: usize) -> u32 {
    let mut value = jlb(s, n);
    while !j1_le(s, n, value) { value += 1; }
    value
}
fn use_j1() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_J1").map(|v| v == "1").unwrap_or(false))
}
static J1REJECT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// J1 を使う子の深さの範囲（COLORLB_J1MIN〜COLORLB_J1MAX、既定は全部）。g は親の深さ。
pub(crate) fn use_j1g(g: usize) -> bool {
    static V: std::sync::OnceLock<(usize, usize)> = std::sync::OnceLock::new();
    let (lo, hi) = *V.get_or_init(|| {
        let f = |k: &str, d: usize| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        (f("COLORLB_J1MIN", 0), f("COLORLB_J1MAX", usize::MAX))
    });
    use_j1() && g + 1 >= lo && g + 1 <= hi
}
fn j1_ok(s: &State, n: usize, thr: u32) -> bool {
    let pass = j1_le(s, n, thr);
    if !pass { J1REJECT.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
    pass
}
fn j1_log_suffix() -> String {
    if use_j1() { format!(" J1 rejected {} aborted {}", J1REJECT.load(std::sync::atomic::Ordering::Relaxed), J1ABORT.load(std::sync::atomic::Ordering::Relaxed)) }
    else { String::new() }
}
fn j1file(n: usize, path: &str) -> std::io::Result<()> {
    use std::io::{BufRead, BufReader};
    let (mut count, mut jdiff, mut j1diff) = (0, 0, 0);
    let mut mismatches = Vec::new();
    for (line_no, line) in BufReader::new(std::fs::File::open(path)?).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let parse_error = || std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}:{}: expected d j j1 lane / lane / ...", path, line_no + 1));
        let mut fields = line.split_whitespace();
        let mut expected = [0u32; 3];
        for v in &mut expected { *v = fields.next().and_then(|s| s.parse().ok()).ok_or_else(parse_error)?; }
        let words: Vec<_> = fields.filter(|&s| s != "/").collect();
        if words.len() > n + 1 || words.is_empty() { return Err(parse_error()); }
        let mut s = [0; MAXL];
        for (i, word) in words.iter().enumerate() {
            if *word == "." { continue; }
            if word.len() > n { return Err(parse_error()); }
            for ch in word.chars() {
                let c = ch.to_digit(16).filter(|&c| c > 0 && c <= n as u32).ok_or_else(parse_error)?;
                s[i] = push(s[i], c as u8);
            }
        }
        let s = canon(&s, n + 1);
        let j = jlb(&s, n);
        let j1 = j1_exact(&s, n);
        count += 1;
        jdiff += (j != expected[1]) as usize;
        j1diff += (j1 != expected[2]) as usize;
        if (j != expected[1] || j1 != expected[2]) && mismatches.len() < 10 {
            mismatches.push(format!("line {}: file J={} J1={}, Rust J={j} J1={j1}: {line}", line_no + 1, expected[1], expected[2]));
        }
    }
    println!("N={n} count={count} J mismatches={jdiff} J1 mismatches={j1diff}");
    for mismatch in mismatches { println!("{mismatch}"); }
    Ok(())
}

/// J は各レーンの（底の色, 底の連続の長さ, その上の色ごとの個数）の多重集合だけで決まる（上の並び順に依らない）。
/// その「形」を鍵にスレッドごとに覚えておく。
static JCALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static JNANOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static JNODES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static JACC: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static JACCNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static JQUICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static JLOOK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// J ≤ thr かどうか（閾値つき）。レーンの形を鍵に、分かった上限と下限をスレッドごとに覚える。
fn jle_cached(s: &State, n: usize, thr: u32) -> bool {
    if use_shared() { return jle_shared(s, n, thr); }
    JLOOK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let key = jkey(s, n);
    thread_local! { static CT: std::cell::RefCell<HashMap<[u64; MAXL], (u8, u8)>> = std::cell::RefCell::new(HashMap::new()); }
    CT.with(|c| {
        let (lo, hi) = c.borrow().get(&key).copied().unwrap_or((0, 255));
        if (hi as u32) <= thr { return true; }
        if (lo as u32) > thr { return false; }
        // 形ごとの記録で決まらないときだけ、先に安い h + D′ で落とす（D′ は形ごとに覚えないので、記録で済む局面には呼ばない）
        if use_dprime() && h1(s, n) + d3(s, n) > thr { return false; }
        let t = std::time::Instant::now();
        // 近道: japprox は J の規則どおりの道 1 本の費用（J 以上）。閾値以下なら DP に入らずに通す。
        let r = { let ja = if use_quick() { japprox(s, n) } else { u32::MAX }; if ja <= thr { JQUICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed); ja } else { jlb_thr(s, n, thr) } };
        JCALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dt = t.elapsed().as_nanos() as u64;
        JNANOS.fetch_add(dt, std::sync::atomic::Ordering::Relaxed);
        if r <= thr { JACC.fetch_add(1, std::sync::atomic::Ordering::Relaxed); JACCNS.fetch_add(dt, std::sync::atomic::Ordering::Relaxed); }
        let mut m = c.borrow_mut();
        if m.len() > 2_000_000 { m.clear(); }
        let e = m.entry(key).or_insert((0, 255));
        if r <= thr { e.1 = e.1.min(r.min(255) as u8); } else { e.0 = e.0.max(r.min(254) as u8); }
        r <= thr
    })
}
/// 形ごとの記録を全スレッドで共有する（既定。COLORLB_NOSHARED を置くとスレッドごとの記録に戻す）。鍵の散らし値で 256 に分けた錠つきの表。
fn use_shared() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("COLORLB_NOSHARED").is_err() && std::env::var("COLORLB_SHARED").map(|v| v != "0").unwrap_or(true))
}
fn jle_shared(s: &State, n: usize, thr: u32) -> bool {
    use std::sync::Mutex;
    static SH: std::sync::OnceLock<Vec<Mutex<HashMap<[u64; MAXL], (u8, u8)>>>> = std::sync::OnceLock::new();
    let sh = SH.get_or_init(|| (0..256).map(|_| Mutex::new(HashMap::new())).collect());
    JLOOK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let key = jkey(s, n);
    let mut hsh = 0u64;
    for &x in &key { hsh = (hsh ^ x).wrapping_mul(0xFF51_AFD7_ED55_8CCD).rotate_left(23); }
    let part = &sh[(hsh >> 56) as usize];
    let (lo, hi) = part.lock().unwrap().get(&key).copied().unwrap_or((0, 255));
    if (hi as u32) <= thr { return true; }
    if (lo as u32) > thr { return false; }
    let t = std::time::Instant::now();
    let r = { let ja = if use_quick() { japprox(s, n) } else { u32::MAX }; if ja <= thr { JQUICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed); ja } else { jlb_thr(s, n, thr) } };
    JCALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dt = t.elapsed().as_nanos() as u64;
    JNANOS.fetch_add(dt, std::sync::atomic::Ordering::Relaxed);
    if r <= thr { JACC.fetch_add(1, std::sync::atomic::Ordering::Relaxed); JACCNS.fetch_add(dt, std::sync::atomic::Ordering::Relaxed); }
    let mut m = part.lock().unwrap();
    // 1 区画 5 万件（全体 1280 万件、約 1.5GB）を超えたらその区画だけ消す
    if m.len() > 50_000 { m.clear(); }
    let e = m.entry(key).or_insert((0, 255));
    if r <= thr { e.1 = e.1.min(r.min(255) as u8); } else { e.0 = e.0.max(r.min(254) as u8); }
    r <= thr
}
fn jkey(s: &State, n: usize) -> [u64; MAXL] {
    let mut key = [0u64; MAXL];
    for (i, &l) in s[..=n].iter().enumerate() {
        let ht = height(l);
        if ht == 0 { continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        let mut sig = (c0 as u64) << 60 | (r as u64) << 56;
        for k in r..ht { sig += 1u64 << (4 * (ball(l, k) as u64 - 1)); }
        key[i] = sig;
    }
    key[..=n].sort_unstable();
    key
}
fn jlb_cached(s: &State, n: usize) -> u32 {
    JLOOK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let key = jkey(s, n);
    thread_local! { static C: std::cell::RefCell<HashMap<[u64; MAXL], u8>> = std::cell::RefCell::new(HashMap::new()); }
    C.with(|c| {
        if let Some(&v) = c.borrow().get(&key) { return v as u32; }
        let t = std::time::Instant::now();
        let v = if use_jref() { jlb_ref(s, n) } else { jlb(s, n) };
        JCALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        JNANOS.fetch_add(t.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        let mut m = c.borrow_mut();
        if m.len() > 2_000_000 { m.clear(); }
        m.insert(key, v.min(255) as u8);
        v
    })
}
/// J の近似（ビーム用。許容性は要らない）。色の家の集合 O を表に持たず、レーンの集合 mask だけの DP にする。
/// 各 mask では費用最小の道の O だけを持ち、「全部出す」で足す色は、残りのレーンに最も多く残っている色を貪欲に選ぶ。
/// 最初の O: 単色レーンは色ごとに 1 本（一番高いもの）を残し、空レーン e 本ぶんは汚れたレーンに多い色から選ぶ。
fn japprox(s: &State, n: usize) -> u32 {
    type It = (u8, u8, [u8; 16], u8, [u8; 16], bool);
    let mut e = 0usize;
    let mut its: Vec<It> = Vec::new();
    let mut uni: Vec<(u8, u8)> = Vec::new();
    for &l in &s[..=n] {
        let ht = height(l);
        if ht == 0 { e += 1; continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        if r == ht { uni.push((c0, ht as u8)); continue; }
        let mut keep = [0u8; 16];
        for k in r..ht { keep[ball(l, k) as usize] += 1; }
        let mut full = keep;
        full[c0 as usize] += r as u8;
        its.push((c0, (ht - r) as u8, keep, ht as u8, full, true));
    }
    // 単色レーン: 色ごとに一番高い 1 本を残し、ほかは順番に片付ける対象に
    uni.sort_unstable_by(|a, b| (a.0, std::cmp::Reverse(a.1)).cmp(&(b.0, std::cmp::Reverse(b.1))));
    let mut o0 = 0u32;
    for &(u, len) in &uni {
        if o0 >> u & 1 == 0 { o0 |= 1 << u; } else {
            let mut full = [0u8; 16];
            full[u as usize] = len;
            its.push((u, 0, [0u8; 16], len, full, false));
        }
    }
    let d = its.len();
    if d == 0 { return 0; }
    let mut tot = [0u32; 16];
    for it in &its { for c in 1..=n { tot[c] += it.4[c] as u32; } }
    // 空レーンの家: 汚れたレーンに多い色から e 色（底の色として「残す」に使えそうな色は後回し）
    let mut cand: Vec<usize> = (1..=n).filter(|&c| o0 >> c & 1 == 0 && tot[c] > 0).collect();
    cand.sort_unstable_by_key(|&c| std::cmp::Reverse(tot[c]));
    for &c in cand.iter().take(e) { o0 |= 1 << c; }
    let cost = |cnt: &[u8; 16], o: u32| -> u32 { let mut x = 0; for k in 1..=n { if o >> k & 1 == 0 { x += cnt[k] as u32; } } x };
    let full_mask = (1usize << d) - 1;
    let mut best: Vec<(u32, u32)> = vec![(u32::MAX, 0); 1 << d];
    best[0] = (0, o0);
    for mask in 0..full_mask {
        let (c, o) = best[mask];
        if c == u32::MAX { continue; }
        // 残りのレーンにある色の個数（足す色の選択用）
        let mut rem = [0u32; 16];
        for i in 0..d { if mask >> i & 1 == 0 { for k in 1..=n { rem[k] += its[i].4[k] as u32; } } }
        for i in 0..d {
            if mask >> i & 1 == 1 { continue; }
            let (c0, nk, ref keep, nf, ref full, cankeep) = its[i];
            let nm = mask | 1 << i;
            if cankeep && o >> c0 & 1 == 0 {
                let v = c + nk as u32 + cost(keep, o);
                if v < best[nm].0 { best[nm] = (v, o | 1 << c0); }
            }
            let v = c + nf as u32 + cost(full, o);
            if v < best[nm].0 {
                let mut pick = 0usize;
                let mut bc = 0u32;
                for k in 1..=n {
                    if o >> k & 1 == 1 { continue; }
                    let r = rem[k] - full[k] as u32;
                    if r > bc { bc = r; pick = k; }
                }
                best[nm] = (v, if pick > 0 { o | 1 << pick } else { o });
            }
        }
    }
    best[full_mask].0
}
fn use_japprox() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_JAPPROX").map(|v| v == "1").unwrap_or(false))
}
/// COLORLB_JREF=1 で以前の J（レーンを 1 本ずつ区別する版）を使う（速さの比較用）。
fn use_jref() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_JREF").is_ok())
}
/// 証明の J の判定: 既定は閾値で打ち切る版。COLORLB_JEXACT=1 で J を正確に求めて比べる（以前の動き）。
fn j_ok(s: &State, n: usize, thr: u32) -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *F.get_or_init(|| std::env::var("COLORLB_JEXACT").is_ok()) { jlb_cached(s, n) <= thr } else { jle_cached(s, n, thr) }
}
fn use_order() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_NOORDER").is_err())
}
fn use_quick() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_NOQUICK").is_err())
}
fn use_sym() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_NOSYM").is_err())
}
fn late_dprime() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_EARLYDP").is_err())
}
fn use_dom() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("COLORLB_NODOM").is_err())
}
/// J の内側の下限を強める（LOWER_BOUND_PROOF.md の 10 番）。COLORLB_NOHB で切る（比べる用）。
fn use_hb() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_NOHB").is_err())
}
fn use_hdyn() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_NOHDYN").is_err())
}
fn use_joint() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var("COLORLB_JOINT").map(|v| v == "1").unwrap_or(false))
}

/// 2 回以上動く玉の数の下界 D
fn d2(s: &State, n: usize) -> u32 {
    let lanes = n + 1;
    let mut base = 0usize;
    // 汚れたレーン: (底の色, keep の表, full の表)
    let mut dl: Vec<(u8, [u8; 16], [u8; 16])> = Vec::new();
    let mut uniform_color = [false; 16];
    for &l in &s[..lanes] {
        let ht = height(l);
        if ht == 0 { base += 1; continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        if r == ht { base += 1; uniform_color[c0 as usize] = true; continue; }
        let mut cnt = [0u8; 16];
        for k in r..ht { cnt[ball(l, k) as usize] += 1; }
        let dirty = (ht - r) as u8;
        let mut fullcnt = cnt;
        fullcnt[c0 as usize] += r as u8;
        let tab = |cnt: &[u8; 16], total: u8| -> [u8; 16] {
            let mut v: Vec<u8> = cnt[1..=n].iter().copied().filter(|&x| x > 0).collect();
            v.sort_unstable_by(|a, b| b.cmp(a));
            let mut t = [0u8; 16];
            let mut acc = 0u8;
            for m in 0..16 {
                if m > 0 && m - 1 < v.len() { acc += v[m - 1]; }
                t[m] = total.saturating_sub(acc);
            }
            t
        };
        dl.push((c0, tab(&cnt, dirty), tab(&fullcnt, dirty + r as u8)));
    }
    let d = dl.len();
    if d == 0 { return 0; }
    // 底の色ごとの組: 残すレーンを 1 本選ぶ（単色レーンがあればそれを残す選択肢もある）
    let mut groups: Vec<(u8, Vec<usize>)> = Vec::new();
    for (i, x) in dl.iter().enumerate() {
        match groups.iter_mut().find(|g| g.0 == x.0) {
            Some(g) => g.1.push(i),
            None => groups.push((x.0, vec![i])),
        }
    }
    // 各組の選択肢: 汚れたレーン i を残す / 単色レーンを残す（usize::MAX）
    let opts: Vec<Vec<usize>> = groups
        .iter()
        .map(|g| {
            let mut o = g.1.clone();
            if uniform_color[g.0 as usize] { o.push(usize::MAX); }
            o
        })
        .collect();
    let combos: usize = opts.iter().map(|o| o.len()).product();
    let relax = combos > 512;
    let mut best = u32::MAX;
    let mut idx = vec![0usize; opts.len()];
    let mut cost = vec![[0u8; 16]; d];
    let mut dp = vec![0u32; 1 << d];
    loop {
        for i in 0..d {
            let keep = if relax {
                true
            } else {
                let g = groups.iter().position(|g| g.0 == dl[i].0).unwrap();
                opts[g][idx[g]] == i
            };
            cost[i] = if relax {
                let mut t = [0u8; 16];
                for m in 0..16 { t[m] = dl[i].1[m].min(dl[i].2[m]); }
                t
            } else if keep { dl[i].1 } else { dl[i].2 };
        }
        // dp[mask] = mask のレーンを最初の |mask| 段に並べたときの最小
        dp[0] = 0;
        for mask in 1usize..(1 << d) {
            let k = mask.count_ones() as usize; // この段のレーンは k 番目
            let m = (base + k - 1).min(15);
            let mut v = u32::MAX;
            let mut rest = mask;
            while rest != 0 {
                let i = rest.trailing_zeros() as usize;
                rest &= rest - 1;
                let c = dp[mask ^ (1 << i)] + cost[i][m] as u32;
                if c < v { v = c; }
            }
            dp[mask] = v;
        }
        best = best.min(dp[(1 << d) - 1]);
        if relax { break; }
        // 次の組み合わせ
        let mut j = 0;
        loop {
            if j == opts.len() { return best; }
            idx[j] += 1;
            if idx[j] < opts[j].len() { break; }
            idx[j] = 0;
            j += 1;
        }
    }
    best
}

fn lb(s: &State, n: usize) -> u32 { h1(s, n) + d2(s, n) }

/// 完成形からの幅優先で正確な距離を求め、h + D と比べる
fn check(n: usize, depth: usize) {
    let g = goal(n);
    let mut dist: HashMap<State, u8> = HashMap::new();
    dist.insert(g, 0);
    let mut layer = vec![g];
    let mut buf = Vec::new();
    for d in 1..=depth {
        let mut next = Vec::new();
        for s in &layer {
            neighbors(s, n, &mut buf);
            // 逆向きも同じ手（可逆）。ただし「1 個だけのレーンから空レーン」の除外は形が変わらないので影響なし
            for t in &buf {
                if !dist.contains_key(t) {
                    dist.insert(*t, d as u8);
                    next.push(*t);
                }
            }
        }
        if next.is_empty() { break; }
        eprintln!("dist {d}: {} states", next.len());
        layer = next;
    }
    let all: Vec<(&State, &u8)> = dist.iter().collect();
    let res: Vec<(i32, u32, u32, u64)> = all
        .par_iter()
        .map(|(s, &d)| {
            let (a, mut b) = (h1(s, n), if use_dprime() { d2(s, n).max(d3(s, n)) } else { d2(s, n) });
            if use_joint() { let j = jlb(s, n); if j > a + b { b = j - a; } }
            ((a + b) as i32 - d as i32, a, b, 1)
        })
        .collect();
    let viol = res.iter().filter(|x| x.0 > 0).count();
    let worst = res.iter().map(|x| x.0).max().unwrap();
    let exact = res.iter().filter(|x| x.0 == 0).count();
    let gain: u64 = res.iter().map(|x| x.2 as u64).sum();
    let avg_gap_h: f64 = all.iter().map(|(s, &d)| d as f64 - h1(s, n) as f64).sum::<f64>() / all.len() as f64;
    let avg_gap_hd: f64 = res.iter().zip(all.iter()).map(|(x, (_, &d))| d as f64 - (x.1 + x.2) as f64).sum::<f64>() / all.len() as f64;
    let s0 = canon(&initial(n), n + 1);
    println!(
        "N={n} states={} violations={viol} worst_excess={worst} exact={exact} sumD={gain} avg(dist-h)={avg_gap_h:.3} avg(dist-h-D)={avg_gap_hd:.3} init: dist={:?} h={} D={}",
        all.len(), dist.get(&s0), h1(&s0, n), d2(&s0, n)
    );
}

/// 状態を 128 ビットに詰める（N ≤ 6）。レーンは (N+1) 進で N マス、レーンの符号を整列して並べる。
fn pack(s: &State, n: usize) -> u128 {
    let b = (n + 1) as u32;
    let bits = (32 - (b.pow(n as u32) - 1).leading_zeros()) as usize;
    let mut codes: Vec<u32> = s[..=n]
        .iter()
        .map(|&l| {
            let mut c = 0u32;
            for k in (0..height(l)).rev() { c = c * b + ball(l, k) as u32; }
            c
        })
        .collect();
    codes.sort_unstable();
    let mut x = 0u128;
    for c in codes { x = (x << bits) | c as u128; }
    x
}
fn unpack(mut x: u128, n: usize) -> State {
    let b = (n + 1) as u32;
    let bits = (32 - (b.pow(n as u32) - 1).leading_zeros()) as usize;
    let mut s = [0u64; MAXL];
    for i in (0..=n).rev() {
        let mut c = (x & ((1u128 << bits) - 1)) as u32;
        x >>= bits;
        let mut l = 0u64;
        while c > 0 { l = push(l, (c % b) as u8); c /= b; }
        s[i] = l;
    }
    canon(&s, n + 1)
}

/// g + h + D ≤ B−1 の状態だけを幅優先でたどる。完成形に届かなければ T(N) ≥ B。
fn prove(n: usize, b: u32) {
    assert!(n <= 6, "128 ビットに詰めるのは 6 色まで");
    let t0 = std::time::Instant::now();
    let s0 = canon(&initial(n), n + 1);
    if lb(&s0, n) > b - 1 {
        println!("N={n} B={b}: 初期状態で h+D={} > {} → T ≥ {b}", lb(&s0, n), b - 1);
        return;
    }
    assert_eq!(pack(&unpack(pack(&s0, n), n), n), pack(&s0, n));
    // 手は逆に指せるので、g+1 層の候補が既出なら g 層か g−1 層にある。枝刈りの条件 g + lb ≤ B−1 は g に依るが、
    // g が小さいほど通りやすいので、深さ g′ ≤ g−2 で出た状態の隣は深さ g′+1 ですでに出ている
    // （LOWER_BOUND_PROOF.md）。全既訪の集合を持たず、整列済みの直近 2 層だけで重複を除く。
    let mut prev: Vec<u128> = Vec::new();
    let mut layer: Vec<u128> = vec![pack(&s0, n)];
    let mut total = 1u64;
    for g in 1..b {
        // 層を 400 万状態ずつに分けて展開し、塊ごとに直近 2 層との重複を除いて整列・重複除去してから足す。
        // 重複込みの候補を一度に全部ためると、B=52 の 26 層目で 9 GB の確保に失敗した。
        let mut next: Vec<u128> = Vec::new();
        for chunk in layer.chunks(4_000_000) {
            let mut part: Vec<u128> = chunk
                .par_iter()
                .fold(Vec::new, |mut acc, &x| {
                    let s = unpack(x, n);
                    let mut buf = Vec::new();
                    neighbors(&s, n, &mut buf);
                    for t in buf {
                        if g + lb(&t, n) <= b - 1 && (late_dprime() || !use_dprime() || g + h1(&t, n) + d3(&t, n) <= b - 1) {
                            let k = pack(&t, n);
                            if layer.binary_search(&k).is_err() && prev.binary_search(&k).is_err() { acc.push(k); }
                        }
                    }
                    acc
                })
                .reduce(Vec::new, |mut a, mut x| { a.append(&mut x); a });
            part.par_sort_unstable();
            part.dedup();
            // D′ と J は、重複を除いたあとにだけ呼ぶ（安い順に D′ → J）
            if (late_dprime() && use_dprime()) || use_joint() {
                part = part.into_par_iter().filter(|k| {
                    let t = unpack(*k, n);
                    (use_joint() || !(late_dprime() && use_dprime()) || g + h1(&t, n) + d3(&t, n) <= b - 1) && (!use_joint() || (j_ok(&t, n, b - 1 - g) && (!use_j1g(g as usize) || j1_ok(&t, n, b - 1 - g))))
                }).collect();
            }
            next.append(&mut part);
            if next.len() > 50_000_000 { next.par_sort_unstable(); next.dedup(); next.shrink_to_fit(); }
        }
        next.par_sort_unstable();
        next.dedup();
        next.shrink_to_fit();
        if next.iter().any(|&t| is_goal(&unpack(t, n), n)) {
            println!("N={n} B={b}: {g} 手で完成に届いた → T ≤ {g}");
            return;
        }
        total += next.len() as u64;
        eprintln!("g={g} layer={} total={total} {:.1}s J呼び出し {} 計算 {} 計 {:.1}s 節点 {} 通過 {} {:.1}s 近道 {}{}", next.len(), t0.elapsed().as_secs_f64(), JLOOK.load(std::sync::atomic::Ordering::Relaxed), JCALLS.load(std::sync::atomic::Ordering::Relaxed), JNANOS.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9, JNODES.load(std::sync::atomic::Ordering::Relaxed), JACC.load(std::sync::atomic::Ordering::Relaxed), JACCNS.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9, JQUICK.load(std::sync::atomic::Ordering::Relaxed), j1_log_suffix());
        if next.is_empty() { break; }
        prev = std::mem::replace(&mut layer, next);
    }
    println!("N={n} B={b}: {}手以内の解なし → T ≥ {b}（たどった状態 {total}、{:.1}s）", b - 1, t0.elapsed().as_secs_f64());
}

/// 緩めた完成条件: 各レーンが空か単色で、同じ色が 2 本のレーンに分かれていない（満杯でなくてよい）。
/// 削除の議論で出る R(m)（m 色・各 m 個・レーン m+2 本・容量 m+1）を解くのに使う。
fn is_goal_relaxed(s: &State, n: usize) -> bool {
    let mut seen = 0u64;
    for &l in &s[..=n] {
        let h = height(l);
        if h == 0 { continue; }
        let c = ball(l, 0);
        if (0..h).any(|k| ball(l, k) != c) { return false; }
        if seen >> c & 1 == 1 { return false; }
        seen |= 1 << c;
    }
    true
}
/// 緩めた完成条件での A*（h だけ使う。h は緩めた条件でも許容的）。
fn ropt(n: usize, s0: State) -> u32 {
    use std::collections::{BinaryHeap, HashMap};
    use std::cmp::Reverse;
    let mut best: HashMap<u128, u32> = HashMap::new();
    let k0 = pack(&s0, n);
    best.insert(k0, 0);
    let mut pq = BinaryHeap::new();
    pq.push(Reverse((h1(&s0, n), 0u32, k0)));
    let mut buf = Vec::new();
    while let Some(Reverse((_f, g, k))) = pq.pop() {
        if best[&k] < g { continue; }
        let s = unpack(k, n);
        if is_goal_relaxed(&s, n) { return g; }
        neighbors(&s, n, &mut buf);
        for t in buf.drain(..) {
            let kt = pack(&t, n);
            let e = best.entry(kt).or_insert(u32::MAX);
            if g + 1 < *e { *e = g + 1; pq.push(Reverse((g + 1 + h1(&t, n), g + 1, kt))); }
        }
    }
    u32::MAX
}

/// 7 色以上の詰め方。各レーンを (N+1) 進の符号にして整列し、64W ビットに詰める。
/// W は色数で選ぶ（7・8 色 4、9 色 5、10 色 7、11 色 8、12 色 10）。
fn lane_bits(n: usize) -> usize { let b = (n + 1) as u64; (64 - (b.pow(n as u32) - 1).leading_zeros()) as usize }
fn pack_w<const W: usize>(s: &State, n: usize) -> [u64; W] {
    let b = (n + 1) as u64;
    let bits = lane_bits(n);
    let mut codes: Vec<u64> = s[..=n]
        .iter()
        .map(|&l| {
            let mut c = 0u64;
            for k in (0..height(l)).rev() { c = c * b + ball(l, k) as u64; }
            c
        })
        .collect();
    codes.sort_unstable();
    let mut x = [0u64; W];
    let mut pos = 0usize;
    for &c in &codes {
        for t in 0..bits {
            if c >> t & 1 == 1 { x[(pos + t) / 64] |= 1 << ((pos + t) % 64); }
        }
        pos += bits;
    }
    x
}
fn unpack_w<const W: usize>(x: &[u64; W], n: usize) -> State {
    let b = (n + 1) as u64;
    let bits = lane_bits(n);
    let mut s = [0u64; MAXL];
    for i in 0..=n {
        let mut c = 0u64;
        for t in 0..bits {
            let p = i * bits + t;
            if x[p / 64] >> (p % 64) & 1 == 1 { c |= 1 << t; }
        }
        let mut l = 0u64;
        while c > 0 { l = push(l, (c % b) as u8); c /= b; }
        s[i] = l;
    }
    canon(&s, n + 1)
}
fn prove4(n: usize, b: u32) {
    match n {
        7 | 8 => prove_w::<4>(n, b),
        9 => prove_w::<5>(n, b),
        10 => prove_w::<7>(n, b),
        11 => prove_w::<8>(n, b),
        12 => prove_w::<10>(n, b),
        _ => panic!("7〜12 色だけ"),
    }
}
/// prove の 7 色以上版（中身は同じ。重複除去は直近 2 層、層を塊に分けて展開）。
fn prove_w<const W: usize>(n: usize, b: u32) {
    assert!((n + 1) * lane_bits(n) <= 64 * W);
    let t0 = std::time::Instant::now();
    let s0 = canon(&initial(n), n + 1);
    let lb0 = h1(&s0, n) + d2(&s0, n).max(if use_dprime() { d3(&s0, n) } else { 0 });
    println!("N={n} B={b}: 初期状態の下界 {lb0}");
    assert_eq!(unpack_w::<W>(&pack_w::<W>(&s0, n), n), s0);
    let mut prev: Vec<[u64; W]> = Vec::new();
    let mut layer: Vec<[u64; W]> = vec![pack_w::<W>(&s0, n)];
    let mut total = 1u64;
    for g in 1..b {
        let mut next: Vec<[u64; W]> = Vec::new();
        for chunk in layer.chunks(2_000_000) {
            let mut part: Vec<[u64; W]> = chunk
                .par_iter()
                .fold(Vec::new, |mut acc, x| {
                    let s = unpack_w::<W>(x, n);
                    let mut buf = Vec::new();
                    neighbors(&s, n, &mut buf);
                    for t in buf {
                        if g + lb(&t, n) <= b - 1 && (late_dprime() || !use_dprime() || g + h1(&t, n) + d3(&t, n) <= b - 1) {
                            let k = pack_w::<W>(&t, n);
                            if layer.binary_search(&k).is_err() && prev.binary_search(&k).is_err() { acc.push(k); }
                        }
                    }
                    acc
                })
                .reduce(Vec::new, |mut a, mut x| { a.append(&mut x); a });
            part.par_sort_unstable();
            part.dedup();
            // D′ と J は、重複を除いたあとにだけ呼ぶ（安い順に D′ → J）
            if (late_dprime() && use_dprime()) || use_joint() {
                part = part.into_par_iter().filter(|k| {
                    let t = unpack_w::<W>(k, n);
                    (use_joint() || !(late_dprime() && use_dprime()) || g + h1(&t, n) + d3(&t, n) <= b - 1) && (!use_joint() || (j_ok(&t, n, b - 1 - g) && (!use_j1g(g as usize) || j1_ok(&t, n, b - 1 - g))))
                }).collect();
            }
            next.append(&mut part);
            if next.len() > 30_000_000 { next.par_sort_unstable(); next.dedup(); next.shrink_to_fit(); }
        }
        next.par_sort_unstable();
        next.dedup();
        next.shrink_to_fit();
        if next.iter().any(|t| is_goal(&unpack_w::<W>(t, n), n)) {
            println!("N={n} B={b}: {g} 手で完成に届いた → T ≤ {g}");
            return;
        }
        total += next.len() as u64;
        eprintln!("g={g} layer={} total={total} {:.1}s J呼び出し {} 計算 {} 計 {:.1}s 節点 {} 通過 {} {:.1}s 近道 {}{}", next.len(), t0.elapsed().as_secs_f64(), JLOOK.load(std::sync::atomic::Ordering::Relaxed), JCALLS.load(std::sync::atomic::Ordering::Relaxed), JNANOS.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9, JNODES.load(std::sync::atomic::Ordering::Relaxed), JACC.load(std::sync::atomic::Ordering::Relaxed), JACCNS.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e9, JQUICK.load(std::sync::atomic::Ordering::Relaxed), j1_log_suffix());
        if next.is_empty() { break; }
        prev = std::mem::replace(&mut layer, next);
    }
    println!("N={n} B={b}: {}手以内の解なし → T ≥ {b}（たどった状態 {total}、{:.1}s）", b - 1, t0.elapsed().as_secs_f64());
}

/// 評価 g + J の層ビーム（fork 側の実験 B）。レーンは実際の番号のまま持ち、重複の判定だけ整列形で行う。
/// 直近 window 層に出た形には戻らない。完成に届いたら手順（36 進 2 文字で 1 手）を出し、最初から再生して確かめる。
fn jbeam(n: usize, width: usize, window: usize) {
    use std::collections::{HashSet, VecDeque};
    const D36: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let t0 = std::time::Instant::now();
    let s0 = initial(n);
    let mut hist: Vec<Vec<(u32, u8, u8)>> = vec![vec![(0, 0, 0)]];
    let mut layer: Vec<State> = vec![s0];
    let mut recent: VecDeque<HashSet<State>> = VecDeque::new();
    recent.push_back([canon(&s0, n + 1)].into_iter().collect());
    for g in 1..400u32 {
        let cand: Vec<(u32, u64, State, u32, u8, u8)> = layer
            .par_iter()
            .enumerate()
            .fold(Vec::new, |mut acc, (pi, s)| {
                let lanes = n + 1;
                let first_empty = (0..lanes).find(|&i| height(s[i]) == 0);
                for a in 0..lanes {
                    let ha = height(s[a]);
                    if ha == 0 { continue; }
                    for b in 0..lanes {
                        if a == b { continue; }
                        let hb = height(s[b]);
                        if hb >= n { continue; }
                        if hb == 0 && (Some(b) != first_empty || ha == 1) { continue; }
                        let t = apply(s, a, b);
                        let k = canon(&t, lanes);
                        if recent.iter().any(|h| h.contains(&k)) { continue; }
                        let j = if use_japprox() { japprox(&t, n) } else { jlb_cached(&t, n) };
                        let mut hsh = 0xcbf29ce484222325u64;
                        for &x in &k[..lanes] { hsh = (hsh ^ x).wrapping_mul(0x100000001b3); }
                        acc.push((g + j, hsh, t, pi as u32, a as u8, b as u8));
                    }
                }
                acc
            })
            .reduce(Vec::new, |mut a, mut x| { a.append(&mut x); a });
        let mut cand = cand;
        cand.par_sort_unstable_by(|x, y| (x.0, x.1).cmp(&(y.0, y.1)));
        let mut seen: HashSet<State> = HashSet::new();
        let mut next: Vec<State> = Vec::new();
        let mut h: Vec<(u32, u8, u8)> = Vec::new();
        let mut best_f = u32::MAX;
        for c in cand {
            if next.len() >= width { break; }
            let k = canon(&c.2, n + 1);
            if !seen.insert(k) { continue; }
            if best_f == u32::MAX { best_f = c.0; }
            next.push(c.2);
            h.push((c.3, c.4, c.5));
        }
        hist.push(h);
        if let Some(i) = next.iter().position(|t| is_goal(&canon(t, n + 1), n)) {
            let mut moves = Vec::new();
            let mut idx = i as u32;
            for gg in (1..=g as usize).rev() {
                let (p, a, b) = hist[gg][idx as usize];
                moves.push((a, b));
                idx = p;
            }
            moves.reverse();
            let mut st = initial(n);
            let mut ok = true;
            for &(a, b) in &moves {
                if height(st[a as usize]) == 0 || height(st[b as usize]) >= n { ok = false; break; }
                st = apply(&st, a as usize, b as usize);
            }
            ok = ok && is_goal(&canon(&st, n + 1), n);
            let sol: String = moves.iter().flat_map(|&(a, b)| [D36[a as usize] as char, D36[b as usize] as char]).collect();
            println!("N={n} width={width} moves={g} verified={ok} {:.1}s", t0.elapsed().as_secs_f64());
            println!("SOL {sol}");
            return;
        }
        eprintln!("g={g} layer={} best_f={best_f} {:.1}s", next.len(), t0.elapsed().as_secs_f64());
        recent.push_back(seen);
        if recent.len() > window { recent.pop_front(); }
        layer = next;
    }
    println!("N={n} width={width}: 400 手で届かず");
}

fn parse_state(n: usize, args: &[String]) -> State {
    let mut s = [0u64; MAXL];
    for (i, w) in args.iter().enumerate() {
        if w == "." { continue; }
        for ch in w.chars() { s[i] = push(s[i], ch.to_digit(16).unwrap() as u8); }
    }
    assert_eq!(args.len(), n + 1);
    canon(&s, n + 1)
}
fn show(s: &State, n: usize) -> String {
    s[..=n].iter().map(|&l| if height(l) == 0 { ".".to_string() } else { (0..height(l)).map(|k| format!("{:x}", ball(l, k))).collect::<String>() }).collect::<Vec<_>>().join(" ")
}
/// A*（h+D は許容的）で任意局面からの最短手数と手順を出す。
fn opt(n: usize, s0: State, quiet: bool) -> u32 {
    use std::collections::{BinaryHeap, HashMap};
    use std::cmp::Reverse;
    let mut best: HashMap<u128, (u32, u128)> = HashMap::new();
    let k0 = pack(&s0, n);
    best.insert(k0, (0, k0));
    let mut pq = BinaryHeap::new();
    pq.push(Reverse((lb(&s0, n), 0u32, k0)));
    let mut buf = Vec::new();
    while let Some(Reverse((_f, g, k))) = pq.pop() {
        if best[&k].0 < g { continue; }
        let s = unpack(k, n);
        if is_goal(&s, n) {
            if !quiet {
                let mut path = vec![k];
                let mut c = k;
                while c != k0 { c = best[&c].1; path.push(c); }
                path.reverse();
                for (i, x) in path.iter().enumerate() { println!("{i:3} {}", show(&unpack(*x, n), n)); }
            }
            return g;
        }
        neighbors(&s, n, &mut buf);
        for t in buf.drain(..) {
            let kt = pack(&t, n);
            let e = best.entry(kt).or_insert((u32::MAX, 0));
            if g + 1 < e.0 {
                *e = (g + 1, k);
                pq.push(Reverse((g + 1 + lb(&t, n), g + 1, kt)));
            }
        }
    }
    u32::MAX
}

/// 初期と完成の両側から幅優先し、g_s + d_g ≤ T + slack の状態を「g_s d_g レーン…」で出す（学習用）。
fn bfs_all(n: usize, src: State) -> HashMap<u128, u8> {
    let mut dist: HashMap<u128, u8> = HashMap::new();
    dist.insert(pack(&src, n), 0);
    let mut layer = vec![pack(&src, n)];
    let mut buf = Vec::new();
    let mut d = 0u8;
    while !layer.is_empty() {
        d += 1;
        let mut next = Vec::new();
        for &x in &layer {
            neighbors(&unpack(x, n), n, &mut buf);
            for t in &buf {
                let k = pack(t, n);
                if !dist.contains_key(&k) { dist.insert(k, d); next.push(k); }
            }
        }
        layer = next;
    }
    dist
}
fn dump(n: usize, slack: u8) {
    let ds = bfs_all(n, canon(&initial(n), n + 1));
    let dg = bfs_all(n, goal(n));
    let t = dg[&pack(&canon(&initial(n), n + 1), n)];
    eprintln!("N={n} T={t} states={}", ds.len());
    for (k, &a) in &ds {
        let b = dg[k];
        let st = unpack(*k, n);
        let r = (*k as u64).wrapping_mul(0x9E3779B97F4A7C15) >> 54; // 0..1023
        if a + b <= t + slack || r < 3 {
            println!("{a} {b} {} {} {}", h1(&st, n), d2(&st, n), show(&st, n));
        }
    }
}

/// 帯の中だけの両側探索: 初期から g+h+D ≤ b の状態を集め、その中で完成から逆向きに距離を取る。
/// g_s + d_g ≤ b の状態を「g_s d_g h D レーン…」で出す。d_g は帯の中の距離（真の距離以上、最適経路上では一致）。
fn band(n: usize, b: u32) {
    let s0 = canon(&initial(n), n + 1);
    let mut gs: HashMap<u128, u8> = HashMap::new();
    gs.insert(pack(&s0, n), 0);
    let mut layer = vec![pack(&s0, n)];
    let mut buf = Vec::new();
    let mut g = 0u32;
    while !layer.is_empty() {
        g += 1;
        let mut next = Vec::new();
        for &x in &layer {
            neighbors(&unpack(x, n), n, &mut buf);
            for t in &buf {
                if g + lb(t, n) > b { continue; }
                let k = pack(t, n);
                if !gs.contains_key(&k) { gs.insert(k, g as u8); next.push(k); }
            }
        }
        layer = next;
    }
    let gk = pack(&goal(n), n);
    eprintln!("band states={} goal_in={}", gs.len(), gs.contains_key(&gk));
    let mut dg: HashMap<u128, u8> = HashMap::new();
    dg.insert(gk, 0);
    let mut layer = vec![gk];
    let mut d = 0u8;
    while !layer.is_empty() {
        d += 1;
        let mut next = Vec::new();
        for &x in &layer {
            neighbors(&unpack(x, n), n, &mut buf);
            for t in &buf {
                let k = pack(t, n);
                if gs.contains_key(&k) && !dg.contains_key(&k) { dg.insert(k, d); next.push(k); }
            }
        }
        layer = next;
    }
    eprintln!("T={:?}", dg.get(&pack(&s0, n)));
    for (k, &a) in &gs {
        if let Some(&c) = dg.get(k) {
            if a as u32 + c as u32 <= b {
                let st = unpack(*k, n);
                println!("{a} {c} {} {} {}", h1(&st, n), d2(&st, n), show(&st, n));
            }
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a[2].parse().unwrap();
    match a[1].as_str() {
        "init" => {
            let s = canon(&initial(n), n + 1);
            println!("N={n} h={} D={} h+D={} formula={}", h1(&s, n), d2(&s, n), lb(&s, n), (3 * n * n - n - 4) / 2);
        }
        "check" => check(n, 1000),
        "near" => check(n, a[3].parse().unwrap()),
        "band" => band(n, a[3].parse().unwrap()),
        "dump" => dump(n, a[3].parse().unwrap()),
        "jbeam" => jbeam(n, a[3].parse().unwrap(), a.get(4).map(|x| x.parse().unwrap()).unwrap_or(6)),
        "ropt" => { let st = parse_state(n, &a[3..]); println!("start {}", show(&st, n)); println!("ropt = {}", ropt(n, st)); }
        "opt" => { let st = parse_state(n, &a[3..]); println!("start {}", show(&st, n)); let g = opt(n, st, false); println!("opt = {g}"); }
        "j1bench" => {
            // 行: thr レーン… （J1STAT の出力から "J1STAT" と pass/nodes/us を除いたもの）。各行の j1_le の結果と時間を出す。
            use std::io::BufRead;
            let f = std::io::BufReader::new(std::fs::File::open(&a[3]).unwrap());
            let mut tot = 0u128;
            for line in f.lines() {
                let line = line.unwrap();
                let w: Vec<&str> = line.split_whitespace().collect();
                if w.len() < 2 { continue; }
                let thr: u32 = w[0].parse().unwrap();
                let mut s = [0; MAXL];
                for (i, word) in w[1..].iter().enumerate() {
                    if *word == "." { continue; }
                    for ch in word.chars() { s[i] = push(s[i], ch.to_digit(16).unwrap() as u8); }
                }
                let s = canon(&s, n + 1);
                let t0 = std::time::Instant::now();
                let r = j1_le(&s, n, thr);
                let us = t0.elapsed().as_micros();
                tot += us;
                println!("{} {} {}", thr, r as u8, us);
            }
            println!("total_us {}", tot);
        }
        "j1file" => {
            assert!((1..MAXL).contains(&n));
            if let Err(error) = j1file(n, &a[3]) {
                eprintln!("j1file failed: {error}");
                std::process::exit(1);
            }
        }
        "jcmp" => {
            // 乱歩の各局面で、形をまとめた J と以前の J が一致するかを数える
            let cnt: usize = a[3].parse().unwrap();
            let mut seed = 987654321u64 + n as u64;
            let (mut same, mut diff) = (0u64, 0u64);
            for _ in 0..cnt {
                let mut s = initial(n);
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let len = (seed >> 33) as usize % (n * n * 2);
                for _ in 0..len {
                    let mut buf = Vec::new();
                    neighbors(&s, n, &mut buf);
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    s = buf[(seed >> 33) as usize % buf.len()];
                }
                let c = canon(&s, n + 1);
                let j = jlb(&c, n);
                let mut ok = j == jlb_ref(&c, n);
                // 閾値つきの J: J−3〜J+3 のどの閾値でも「J ≤ thr」の判定が正確な J と一致し、戻り値も約束どおりか
                for d in 0..7u32 {
                    let thr = (j + d).saturating_sub(3);
                    let r = jlb_thr(&c, n, thr);
                    if (r <= thr) != (j <= thr) || (r <= thr && r < j) || (r > thr && r > j) { ok = false; }
                }
                let ja = japprox(&c, n);
                if ja < j { ok = false; println!("japprox {ja} < J {j}"); }
                if ok { same += 1 } else { diff += 1; println!("不一致 {}", show(&c, n)); }
            }
            println!("N={n} 一致 {same} 不一致 {diff}");
        }
        "jtime" => {
            // 初期状態から決まった乱数で歩いた状態で J の計算時間を測る
            let mut s = initial(n);
            let mut seed = 12345u64;
            let steps: usize = a[3].parse().unwrap();
            for k in 0..=steps {
                if k % (steps / 5).max(1) == 0 {
                    let t = std::time::Instant::now();
                    let v = jlb(&canon(&s, n + 1), n);
                    let dt = t.elapsed().as_secs_f64();
                    let t2 = std::time::Instant::now();
                    let r = if std::env::var("JREF").is_ok() { jlb_ref(&canon(&s, n + 1), n) } else { v };
                    println!("step={k} J={v} ref={r}{} h+D={} {:.3}s (ref {:.3}s)", if r != v { " 不一致" } else { "" }, lb(&canon(&s, n + 1), n), dt, t2.elapsed().as_secs_f64());
                }
                let mut buf = Vec::new();
                neighbors(&s, n, &mut buf);
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                s = buf[(seed >> 33) as usize % buf.len()];
            }
        }
        "prove" => if n <= 6 { prove(n, a[3].parse().unwrap()) } else { prove4(n, a[3].parse().unwrap()) },
        _ => panic!("mode?"),
    }
}

#[cfg(test)]
mod j1_tests {
    use super::*;

    fn state(n: usize, text: &str) -> State {
        parse_state(n, &text.split_whitespace().map(String::from).collect::<Vec<_>>())
    }

    // Independent recurrence: lane identities, every full/open-none action,
    // every colour opening, no shape/symmetry/dominance pruning or bounds.
    fn suffix_exact(p: &J1Problem, mask: usize, o: u32, memo: &mut [Option<u32>]) -> u32 {
        if mask == p.full { return 0; }
        let key = (mask << p.n) | o as usize;
        if let Some(v) = memo[key] { return v; }
        let mut best = u32::MAX;
        for (i, l) in p.lanes.iter().enumerate() {
            if mask >> i & 1 != 0 { continue; }
            let nm = mask | 1 << i;
            if l.dirty && o & l.c0 == 0 {
                best = best.min(l.costs[o as usize][1] as u32 + suffix_exact(p, nm, o | l.c0, memo));
            }
            let cost = l.costs[o as usize][0] as u32;
            best = best.min(cost + suffix_exact(p, nm, o, memo));
            for c in 0..p.n {
                if o >> c & 1 == 0 {
                    best = best.min(cost + suffix_exact(p, nm, o | 1 << c, memo));
                }
            }
        }
        memo[key] = Some(best);
        best
    }

    fn suffix_problem(n: usize, text: &str) -> J1Problem {
        let s = state(n, text);
        J1Problem::new(n, s[..=n].iter().filter(|&&l| l != 0).map(|&l| J1Lane::new(l, n)).collect())
    }

    #[test]
    fn suffix_thresholds_and_witnesses_match_unpruned_recurrence() {
        let mut memo = J1Memo::default();
        for (n, text) in [
            (3, "111 222 333 ."),
            (3, "123 123 123 ."),
            (3, "131 123 23 2"),
            (3, "1 2 21 31"),
            (4, "1234 1243 22 33 ."),
            (4, "1423 14 3312 24 2413"),
            (5, "12345 12543 12345 . . ."),
        ] {
            let mut p = suffix_problem(n, text);
            p.share_min = 0;
            p.share_enabled = true;
            let mut exact = vec![None; p.size];
            memo.reset(p.size);
            // Descending masks visit the same suffix through multiple original
            // lane identities; descending openings also exercises normalization.
            for mask in (0..=p.full).rev() {
                for o in (0..1u32 << n).rev() {
                    let v = suffix_exact(&p, mask, o, &mut exact);
                    assert!(p.lower(mask, o).0 <= v, "lower: {text}, {mask}, {o}");
                    for budget in [v + 2, v.saturating_sub(1), v, 0, v + 1] {
                        assert_eq!(p.go(mask, o, budget, &mut memo), v <= budget,
                            "{text}, mask={mask}, o={o}, budget={budget}, exact={v}");
                        let (lo, hi) = memo.bounds(p.suffix_key(mask, o));
                        assert!(lo <= v && v <= hi, "invalid bounds: {text}, {mask}, {o}: {lo} <= {v} <= {hi}");
                    }
                }
            }
        }
    }

    #[test]
    fn suffix_shared_bounds_survive_new_problem_and_epoch() {
        let mut p = suffix_problem(5, "12345 12345 12345 12345 12345 .");
        let mut q = suffix_problem(5, "12543 12435 12534 12453 . .");
        p.share_min = 0; p.share_enabled = true;
        q.share_min = 0; q.share_enabled = true;
        let key = p.shared_key(1, 2);
        assert_eq!(key, q.shared_key(0, 16));
        let mut exact = vec![None; p.size];
        let v = suffix_exact(&p, 1, 2, &mut exact);
        let mut memo = J1Memo::default();
        memo.reset(p.size);
        assert!(!p.go(1, 2, v - 1, &mut memo));
        assert!(p.go(1, 2, v, &mut memo));
        assert_eq!(memo.shared[&key], (v, v));
        memo.reset(q.size);
        assert!(q.go(0, 16, v, &mut memo));
        assert!(!q.go(0, 16, v - 1, &mut memo));
        assert_eq!(memo.bounds(q.suffix_key(0, 16)), (v, v));
        assert_ne!(key, q.shared_key(0, 1));
        for text in ["11234 12435 12534 12453 . .", "12345 12435 12534 . . .", "11111 12435 12534 12453 . ."] {
            assert_ne!(key, suffix_problem(5, text).shared_key(0, 2));
        }
    }

    #[test]
    fn suffix_shared_table_has_bounded_capacity() {
        let mut memo = J1Memo::default();
        for i in 0..=J1_SHARED_LIMIT {
            let mut key = J1SharedKey { shapes: [0; MAXL], o: 0, n: 5 };
            key.shapes[0] = i as u64;
            memo.record_shared(key, 7, false);
        }
        assert_eq!(memo.shared.len(), 1);
        assert!(memo.shared.capacity() < 2 * J1_SHARED_LIMIT);
        assert!(memo.shared.capacity() * (std::mem::size_of::<J1SharedKey>() + 9) < 24 * 1024 * 1024);
    }

    #[test]
    fn lane_preparation_matches_direct_ball_calculation() {
        for (n, text) in [(3, "131 123 23 2"), (5, "11213 55555 52412 1 . .")] {
            let p = suffix_problem(n, text);
            for l in &p.lanes {
                for o in 0..1usize << n {
                    let mut costs = [0u8; 2];
                    let mut full = [0u8; 16];
                    let mut keep = [0u8; 16];
                    let h = height(l.lane);
                    for pos in 0..h {
                        let c = (ball(l.lane, pos) - 1) as usize;
                        let closed = o >> c & 1 == 0;
                        costs[0] += 1 + closed as u8;
                        if pos >= l.run { costs[1] += 1 + closed as u8; }
                        if closed {
                            full[c] += 1;
                            if pos >= l.run && l.c0 != 1 << c { keep[c] += 1; }
                        }
                    }
                    assert_eq!(l.costs[o], costs);
                    full.sort_unstable_by(|a, b| b.cmp(a));
                    keep.sort_unstable_by(|a, b| b.cmp(a));
                    let cankeep = l.dirty && o & l.c0 as usize == 0;
                    let movement = h - if cankeep { l.run } else { 0 };
                    for q in 0..MAXL {
                        let fc = costs[0] - full[..q].iter().sum::<u8>();
                        let kc = costs[1] - keep[..q].iter().sum::<u8>();
                        let direct = [(if cankeep { fc.min(kc) } else { fc }) - movement as u8,
                            if cankeep { (fc - h as u8).min(kc - (h - l.run) as u8) } else { fc - h as u8 }];
                        assert_eq!(l.relax(o)[q], direct);
                    }
                }
            }
        }
    }

    #[test]
    fn suffix_shape_key_ignores_upper_order_and_symmetric_colour_names() {
        let p = suffix_problem(5, "12345 12543 12345 . . .");
        assert_eq!(p.groups.len(), 1);
        assert_eq!(p.suffix_key(1, 2), p.suffix_key(2, 16));
        assert_ne!(p.suffix_key(1, 1), p.suffix_key(1, 2));
        // A closed bottom colour may still be kept. Opening more colours can
        // increase V, so the suffix must retain the open-nothing action.
        let p = suffix_problem(3, "122 . . .");
        let mut memo = J1Memo::default();
        memo.reset(p.size);
        assert!(p.go(0, 2, 2, &mut memo));
        assert!(!p.go(0, 3, 2, &mut memo));
    }

    #[test]
    fn python_j1_values_and_thresholds() {
        // Values from joint.J and jfirst.J1, with L_cur and N_ set explicitly.
        // Include empty plans, buffers, duplicate uniform colours, and gains.
        for (n, text, j, j1) in [
            (3, "111 222 333 .", 0, 0),
            (3, "123 123 123 .", 10, 10),
            (3, "321 . 323 121", 8, 9),
            (3, "131 123 23 2", 8, 9),
            (3, ". 32 211 .", 3, 3),
            (4, "1423 14 3312 24 2413", 15, 16),
            (4, "2213 1334 3412 412 4", 13, 14),
            (4, "1 2 . 11 33", 1, 1),
            (5, "12345 12345 12345 12345 12345 .", 33, 33),
        ] {
            let s = state(n, text);
            assert_eq!(jlb(&s, n), j, "J: {text}");
            assert_eq!(j1_exact(&s, n), j1, "J1: {text}");
            for thr in (0..=j1 + 2).rev() {
                assert_eq!(j1_le(&s, n, thr), j1 <= thr, "{text}, thr={thr}");
            }
            assert!(j1_le(&s, n, u32::MAX));
        }
    }

    #[test]
    fn upper_order_changes_j1_but_not_j() {
        let a = state(3, ". 121 123 323");
        let b = state(3, ". 121 132 323");
        assert_eq!(jkey(&a, 3), jkey(&b, 3));
        assert_eq!(jlb(&a, 3), 8);
        assert_eq!(jlb(&b, 3), 8);
        // Alternating states also checks that thread-local epochs do not leak.
        for _ in 0..2 {
            assert!(!j1_le(&a, 3, 8));
            assert!(j1_le(&b, 3, 8));
            assert!(j1_le(&a, 3, 9));
        }
    }

    #[test]
    #[ignore = "individual N=7 early-layer timing; run with --release --nocapture"]
    fn j1_early_layers_timing() {
        let n = 7;
        let mut seen = std::collections::BTreeSet::new();
        let mut layer = vec![canon(&initial(n), n + 1)];
        seen.insert(layer[0]);
        let mut buf = Vec::new();
        for depth in 0..=5 {
            let mut next = std::collections::BTreeSet::new();
            let mut total = 0.0;
            let mut worst = 0.0f64;
            for s in &layer {
                let thr = 70 - depth;
                let t = std::time::Instant::now();
                let j = jlb_thr(s, n, thr);
                let j_ms = t.elapsed().as_secs_f64() * 1000.0;
                let t = std::time::Instant::now();
                let pass = j1_le(s, n, thr);
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                assert!(!pass || j <= thr);
                total += ms;
                worst = worst.max(ms);
                println!("depth={depth} thr={thr} J_le={} J_ms={j_ms:.3} J1_le={pass} J1_ms={ms:.3} {}", j <= thr, show(s, n));
                if depth < 5 {
                    neighbors(s, n, &mut buf);
                    for s in &buf { if seen.insert(*s) { next.insert(*s); } }
                }
            }
            println!("depth={depth} count={} J1_total_ms={total:.3} J1_max_ms={worst:.3}", layer.len());
            layer = next.into_iter().collect();
        }
    }
}
