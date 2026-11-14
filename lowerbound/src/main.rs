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
/// J は各レーンの（底の色, 底の連続の長さ, その上の色ごとの個数）の多重集合だけで決まる（上の並び順に依らない）。
/// その「形」を鍵にスレッドごとに覚えておく。
fn jlb_cached(s: &State, n: usize) -> u32 {
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
    thread_local! { static C: std::cell::RefCell<HashMap<[u64; MAXL], u8>> = std::cell::RefCell::new(HashMap::new()); }
    C.with(|c| {
        if let Some(&v) = c.borrow().get(&key) { return v as u32; }
        let v = jlb(s, n);
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
                        if g + lb(&t, n) <= b - 1 && (!use_dprime() || g + h1(&t, n) + d3(&t, n) <= b - 1) {
                            let k = pack(&t, n);
                            if layer.binary_search(&k).is_err() && prev.binary_search(&k).is_err() { acc.push(k); }
                        }
                    }
                    acc
                })
                .reduce(Vec::new, |mut a, mut x| { a.append(&mut x); a });
            part.par_sort_unstable();
            part.dedup();
            if use_joint() { part = part.into_par_iter().filter(|k| g + jlb_cached(&unpack(*k, n), n) <= b - 1).collect(); }
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
        eprintln!("g={g} layer={} total={total} {:.1}s", next.len(), t0.elapsed().as_secs_f64());
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
                        if g + lb(&t, n) <= b - 1 && (!use_dprime() || g + h1(&t, n) + d3(&t, n) <= b - 1) {
                            let k = pack_w::<W>(&t, n);
                            if layer.binary_search(&k).is_err() && prev.binary_search(&k).is_err() { acc.push(k); }
                        }
                    }
                    acc
                })
                .reduce(Vec::new, |mut a, mut x| { a.append(&mut x); a });
            part.par_sort_unstable();
            part.dedup();
            if use_joint() { part = part.into_par_iter().filter(|k| g + jlb_cached(&unpack_w::<W>(k, n), n) <= b - 1).collect(); }
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
        eprintln!("g={g} layer={} total={total} {:.1}s", next.len(), t0.elapsed().as_secs_f64());
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
        "jtime" => {
            // 初期状態から決まった乱数で歩いた状態で J の計算時間を測る
            let mut s = initial(n);
            let mut seed = 12345u64;
            let steps: usize = a[3].parse().unwrap();
            for k in 0..=steps {
                if k % (steps / 5).max(1) == 0 {
                    let t = std::time::Instant::now();
                    let v = jlb(&canon(&s, n + 1), n);
                    println!("step={k} J={v} h+D={} {:.3}s", lb(&canon(&s, n + 1), n), t.elapsed().as_secs_f64());
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
