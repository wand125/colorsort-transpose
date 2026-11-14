//! 色そろえパズルの上界探し（24 色まで用の版。レーンを 128 ビット、色を 5 ビットで持つ）。
//! 中身は colorsort（main.rs）と同じ。
//! 層ごとのビーム探索。評価値は h + λP（Docs 4・5 節）、レーンの並べ替えで状態を同一視する。
//!
//! 使い方: colorsort N WIDTH [--lambda X] [--seed S] [--r1] [--r2] [--noise K] [--max-depth D]
//! 出力: 見つかった手数と、36 進 2 文字/手の文字列（レーン 0 が最初の空レーン）。
use rayon::prelude::*;
use std::collections::HashSet;

const MAXL: usize = 25;
type Lane = u128; // ビット 120〜 = 高さ、下位から 5 ビットずつ底→上の色

#[inline]
fn height(l: Lane) -> usize {
    (l >> 120) as usize
}
#[inline]
fn ball(l: Lane, k: usize) -> u8 {
    ((l >> (5 * k)) & 31) as u8
}
#[inline]
fn top(l: Lane) -> u8 {
    ball(l, height(l) - 1)
}
#[inline]
fn pop(l: Lane) -> Lane {
    let h = height(l);
    let body = l & ((1u128 << (5 * (h - 1))) - 1);
    body | (((h - 1) as u128) << 120)
}
#[inline]
fn push(l: Lane, c: u8) -> Lane {
    let h = height(l);
    let body = l & ((1u128 << 120) - 1);
    body | ((c as u128) << (5 * h)) | (((h + 1) as u128) << 120)
}

#[derive(Clone, Copy)]
struct Cfg {
    n: usize,
    lanes: usize, // 動かすレーン数（R1 なら N、そうでなければ N+1）
    r2: bool,
    lambda: u32,
    mu: u32,
    fin: u32,
    noise: u64,
    seed: u64,
    hmode: bool,
    learned: bool,
    tgt: [u8; 32],
}

type State = [Lane; MAXL];
#[inline]
fn fold(l: Lane) -> u64 { (l as u64) ^ ((l >> 64) as u64).rotate_left(17) }

fn canon(s: &State, c: &Cfg) -> State {
    let mut t = *s;
    t[..c.lanes].sort_unstable();
    t
}

fn hash(s: &State, c: &Cfg) -> u64 {
    let mut h: u64 = 0x9e3779b97f4a7c15 ^ c.seed.wrapping_mul(0x2545F4914F6CDD1D);
    for &l in &s[..c.lanes] {
        h = (h ^ fold(l)).wrapping_mul(0x100000001b3).rotate_left(29);
        h ^= h >> 31;
    }
    h
}
fn key(s: &State, c: &Cfg) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &l in &s[..c.lanes] {
        h ^= fold(l);
        h = h.wrapping_mul(0x100000001b3);
        h ^= h >> 29;
    }
    h
}

/// 残り手数の下界 h と、初期の並び 1,2,3,… が残る分 P。
fn eval(s: &State, c: &Cfg) -> (u32, u32) {
    let mut h = 0u32;
    let mut sum_r = [0u32; 32];
    let mut max_r = [0u32; 32];
    let (mut sk, mut mk) = (0u32, 0u32);
    for &l in &s[..c.lanes] {
        let ht = height(l);
        if ht == 0 {
            continue;
        }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 {
            r += 1;
        }
        h += (ht - r) as u32;
        sum_r[c0 as usize] += r as u32;
        max_r[c0 as usize] = max_r[c0 as usize].max(r as u32);
        let mut k = 0;
        while k < ht && ball(l, k) as usize == k + 1 {
            k += 1;
        }
        sk += k as u32;
        mk = mk.max(k as u32);
    }
    for col in 1..=c.n {
        h += sum_r[col] - max_r[col];
    }
    (h, sk - mk)
}


/// 2 回以上動く玉の数の目安（colorlb の D）。汚れたレーンが 8 本以下なら部分集合 DP、
/// それより多ければ貪欲に並べる。残すレーンは底の同色部分が最長のもの。
fn d2(s: &State, c: &Cfg) -> u32 {
    let n = c.n;
    let mut base = 0usize;
    let mut dl: Vec<(u8, u8, [u8; 32])> = Vec::new(); // (底の色, 長さ, 表 before keeper 決定) → 後で選ぶ
    let mut raw: Vec<(u8, usize, [u8; 32], u8)> = Vec::new(); // (底の色, r, 汚れ部分の色の個数, 汚れの個数)
    let mut uni = [false; 32];
    for &l in &s[..c.lanes] {
        let ht = height(l);
        if ht == 0 { base += 1; continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        if r == ht { base += 1; uni[c0 as usize] = true; continue; }
        let mut cnt = [0u8; 32];
        for k in r..ht { cnt[ball(l, k) as usize] += 1; }
        raw.push((c0, r, cnt, (ht - r) as u8));
    }
    let _ = &mut dl;
    if raw.is_empty() { return 0; }
    // 残すレーン: 色ごとに r 最大の 1 本（単色レーンがあれば残さない）
    let mut keeper = [usize::MAX; 32];
    for (i, x) in raw.iter().enumerate() {
        if uni[x.0 as usize] { continue; }
        let k = &mut keeper[x.0 as usize];
        if *k == usize::MAX || raw[*k].1 < x.1 { *k = i; }
    }
    let tabs: Vec<[u8; 32]> = raw.iter().enumerate().map(|(i, x)| {
        let mut cnt = x.2;
        let mut total = x.3;
        if keeper[x.0 as usize] != i { cnt[x.0 as usize] += x.1 as u8; total += x.1 as u8; }
        let mut v: Vec<u8> = cnt[1..=n].iter().copied().filter(|&y| y > 0).collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        let mut t = [0u8; 32];
        let mut acc = 0u8;
        for m in 0..32 {
            if m > 0 && m - 1 < v.len() { acc += v[m - 1]; }
            t[m] = total.saturating_sub(acc);
        }
        t
    }).collect();
    let d = tabs.len();
    if d <= 8 {
        let mut dp = vec![0u32; 1 << d];
        for mask in 1usize..(1 << d) {
            let m = (base + mask.count_ones() as usize - 1).min(31);
            let mut v = u32::MAX;
            let mut rest = mask;
            while rest != 0 {
                let i = rest.trailing_zeros() as usize;
                rest &= rest - 1;
                v = v.min(dp[mask ^ (1 << i)] + tabs[i][m] as u32);
            }
            dp[mask] = v;
        }
        dp[(1 << d) - 1]
    } else {
        // 貪欲: 早い段ほど損の少ないレーン（今の段と最後の段の差が小さいもの）を置く
        let mut used = vec![false; d];
        let last = (base + d - 1).min(31);
        let mut sum = 0u32;
        for k in 0..d {
            let m = (base + k).min(31);
            let mut bi = 0; let mut bv = i32::MAX;
            for i in 0..d {
                if used[i] { continue; }
                let v = tabs[i][m] as i32 - tabs[i][last] as i32;
                if v < bv { bv = v; bi = i; }
            }
            used[bi] = true;
            sum += tabs[bi][m] as u32;
        }
        sum
    }
}

/// 底から N−1 個以上が同じ色でそろったレーンの数
/// N=4,5 の厳密距離から合わせた補正（100 倍）: 25·D + 50·完成レーン数 + 50·底色の重複。
/// 完成レーンは減点（早く完成させすぎない方が良い）。--learned のときは λ を 0 にして使う。
fn learned_extra(s: &State, c: &Cfg) -> u32 {
    let mut done = 0u32;
    let mut seen = [0u8; 32];
    let mut dup = 0u32;
    for &l in &s[..c.lanes] {
        let ht = height(l);
        if ht == 0 { continue; }
        let c0 = ball(l, 0);
        if ht == c.n && (0..ht).all(|k| ball(l, k) == c0) { done += 1; }
        if seen[c0 as usize] > 0 { dup += 1; }
        seen[c0 as usize] += 1;
    }
    25 * d2(s, c) + 50 * done + 50 * dup
}

fn finished(s: &State, c: &Cfg) -> u32 {
    s[..c.lanes].iter().filter(|&&l| {
        let ht = height(l);
        ht >= c.n - 1 && (0..c.n - 1).all(|k| ball(l, k) == ball(l, 0))
    }).count() as u32
}

fn solved(s: &State, c: &Cfg) -> bool {
    if c.hmode {
        return s[..c.lanes].iter().all(|&l| {
            let ht = height(l);
            ht == 0 || ((0..ht).all(|k| ball(l, k) == ball(l, 0)) && ht == c.tgt[ball(l, 0) as usize] as usize)
        });
    }
    s[..c.lanes].iter().all(|&l| {
        let ht = height(l);
        ht == 0 || (0..ht).all(|k| ball(l, k) == ball(l, 0))
    }) && if c.lanes == c.n + 1 {
        s[..c.lanes].iter().all(|&l| height(l) == 0 || height(l) == c.n)
    } else {
        s[..c.lanes].iter().all(|&l| height(l) == c.n - 1)
    }
}

fn moves(s: &State, c: &Cfg, out: &mut Vec<(u8, u8)>) {
    out.clear();
    let first_empty = (0..c.lanes).find(|&i| height(s[i]) == 0);
    for a in 0..c.lanes {
        let ha = height(s[a]);
        if ha == 0 {
            continue;
        }
        let ca = top(s[a]);
        for b in 0..c.lanes {
            if a == b {
                continue;
            }
            let hb = height(s[b]);
            if hb >= c.n {
                continue;
            }
            if hb == 0 {
                if Some(b) != first_empty || ha == 1 {
                    continue;
                }
            } else if c.r2 && ca > top(s[b]) {
                continue;
            }
            out.push((a as u8, b as u8));
        }
    }
}

fn apply(s: &State, a: usize, b: usize) -> State {
    let mut t = *s;
    let c = top(t[a]);
    t[a] = pop(t[a]);
    t[b] = push(t[b], c);
    t
}

fn initial(n: usize) -> State {
    let mut s = [0u128; MAXL];
    for i in 1..=n {
        let mut l = 0;
        for col in 1..=n {
            l = push(l, col as u8);
        }
        s[i] = l;
    }
    s
}

/// メモリを節約した層ごとのビーム探索。候補は (評価, 鍵, 親, 手) だけで持ち、
/// 残した候補の状態だけを作り直す。履歴は親と手だけを持つ。
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args[1].parse().unwrap();
    let width: usize = args[2].parse().unwrap();
    let mut lambda = 100u32;
    let mut mu = 0u32;
    let mut fin = 0u32;
    let mut seed = 1u64;
    let (mut r1, mut r2) = (false, false);
    let mut hmode = false;
    let mut learned = false;
    let mut start_str: Option<String> = None;
    let mut noise = 0u64;
    let mut max_depth = 10 * n * n;
    let mut window = 6usize;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--lambda" => { lambda = args[i + 1].parse().unwrap(); i += 1; }
            "--fin" => { fin = args[i + 1].parse().unwrap(); i += 1; }
            "--mu" => { mu = args[i + 1].parse().unwrap(); i += 1; }
            "--seed" => { seed = args[i + 1].parse().unwrap(); i += 1; }
            "--noise" => { noise = args[i + 1].parse().unwrap(); i += 1; }
            "--max-depth" => { max_depth = args[i + 1].parse().unwrap(); i += 1; }
            "--window" => { window = args[i + 1].parse().unwrap(); i += 1; }
            "--r1" => r1 = true,
            "--h" => hmode = true,
            "--learned" => learned = true,
            "--start" => { start_str = Some(args[i + 1].clone()); i += 1; }
            "--r2" => r2 = true,
            a => panic!("unknown arg {a}"),
        }
        i += 1;
    }
    // --h: 1 ラウンド目（2N−1 手）で色 N を完成させ、レーン N の玉 v をレーン v に載せる。
    // レーン 1 = [1..N−1, 1] を凍結し、最後に配る。動かすのはレーン 2..N（N−1 本）。
    let mut prefix: Vec<(usize, usize)> = Vec::new();
    let mut real = initial(n);
    let amap: Vec<usize> = (2..=n).collect();
    let mut tgt = [0u8; 32];
    if hmode {
        for i in 1..n { prefix.push((i, 0)); }
        prefix.push((n, 0));
        for v in (1..n).rev() { prefix.push((n, v)); }
        for &(a, b) in &prefix { real = apply(&real, a, b); }
        for c in 1..=n { tgt[c] = n as u8; }
        for k in 0..height(real[1]) { tgt[ball(real[1], k) as usize] -= 1; }
    }
    let cfg = Cfg { n, lanes: if hmode { n - 1 } else if r1 { n } else { n + 1 }, r2, lambda, mu, fin, noise, seed, hmode, learned, tgt };
    let t0 = std::time::Instant::now();
    let start = if hmode {
        let mut s = [0u128; MAXL];
        for (i, &r) in amap.iter().enumerate() { s[i] = real[r]; }
        s
    } else if let Some(ss) = &start_str {
        // 36 進の色をレーンごとにカンマ区切り、空レーンは "."
        let mut s = [0u128; MAXL];
        for (i, w) in ss.split(',').enumerate() {
            if w == "." { continue; }
            for ch in w.chars() { s[i] = push(s[i], ch.to_digit(36).unwrap() as u8); }
        }
        s
    } else { initial(n) };
    let s0 = canon(&start, &cfg);
    let mut layer: Vec<State> = vec![s0];
    let mut hist: Vec<Vec<(u32, u8, u8)>> = vec![vec![(0, 0, 0)]];
    // 直近 window 層の鍵だけを覚えて、戻る手を除く
    let mut recent: std::collections::VecDeque<HashSet<u64>> = std::collections::VecDeque::new();
    recent.push_back([key(&s0, &cfg)].into_iter().collect());
    let mut found: Option<usize> = None;
    for depth in 1..=max_depth {
        let mut cand: Vec<(u64, u64, u32, u8, u8)> = layer
            .par_iter()
            .enumerate()
            .fold(Vec::new, |mut acc, (pi, s)| {
                let mut mv = Vec::with_capacity(160);
                moves(s, &cfg, &mut mv);
                for &(a, b) in &mv {
                    let t = canon(&apply(s, a as usize, b as usize), &cfg);
                    let k = key(&t, &cfg);
                    if recent.iter().any(|h| h.contains(&k)) {
                        continue;
                    }
                    let (h, p) = eval(&t, &cfg);
                    let dd = if cfg.mu > 0 { d2(&t, &cfg) } else { 0 };
                    let done = if cfg.fin > 0 { finished(&t, &cfg) } else { 0 };
                    let extra = if cfg.learned { learned_extra(&t, &cfg) } else { 0 };
                    let score = ((h as u64) * 100 + (p as u64) * cfg.lambda as u64 + (dd as u64) * cfg.mu as u64 + extra as u64 + 1_000_000).saturating_sub(done as u64 * cfg.fin as u64);
                    let nz = if cfg.noise > 0 { hash(&t, &cfg) % cfg.noise } else { 0 };
                    acc.push(((score << 20) + nz.min((1 << 20) - 1), k, pi as u32, a, b));
                }
                acc
            })
            .reduce(Vec::new, |mut a, mut b| { a.append(&mut b); a });
        cand.par_sort_unstable_by_key(|x| (x.1, x.0));
        cand.dedup_by_key(|x| x.1);
        if cand.is_empty() {
            eprintln!("depth {depth}: 候補なし");
            break;
        }
        if cand.len() > width {
            cand.select_nth_unstable_by_key(width - 1, |x| x.0);
            cand.truncate(width);
        }
        cand.par_sort_unstable_by_key(|x| x.0);
        let next: Vec<State> = cand
            .par_iter()
            .map(|x| canon(&apply(&layer[x.2 as usize], x.3 as usize, x.4 as usize), &cfg))
            .collect();
        recent.push_back(cand.iter().map(|x| x.1).collect());
        while recent.len() > window {
            recent.pop_front();
        }
        hist.push(cand.iter().map(|x| (x.2, x.3, x.4)).collect());
        layer = next;
        if depth % 10 == 0 {
            let (bh, bp) = eval(&layer[0], &cfg);
            eprintln!("depth {depth} kept {} best h={bh} P={bp} {:.1}s", layer.len(), t0.elapsed().as_secs_f64());
        }
        if let Some(pos) = layer.iter().position(|s| solved(s, &cfg)) {
            found = Some(pos);
            break;
        }
    }
    let Some(mut pos) = found else {
        println!("NOT FOUND");
        return;
    };
    // 親と手をたどり、正規形の上での手の列を得る
    let mut cmoves = Vec::new();
    for d in (1..hist.len()).rev() {
        let (p, a, b) = hist[d][pos];
        cmoves.push((a, b));
        pos = p as usize;
    }
    cmoves.reverse();
    // 正規形で再生しながら、実際のレーン番号の手に直す
    let mut canon_cur = s0;
    let mut cur = start;
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut mv = Vec::new();
    for &(a, b) in &cmoves {
        let next = canon(&apply(&canon_cur, a as usize, b as usize), &cfg);
        moves(&cur, &cfg, &mut mv);
        let &(ra, rb) = mv
            .iter()
            .find(|&&(x, y)| canon(&apply(&cur, x as usize, y as usize), &cfg)[..cfg.lanes] == next[..cfg.lanes])
            .expect("手の復元に失敗");
        cur = apply(&cur, ra as usize, rb as usize);
        canon_cur = next;
        out.push((ra as usize, rb as usize));
    }
    if hmode {
        let mut full = prefix.clone();
        for &(a, b) in &out { full.push((amap[a], amap[b])); }
        let mut st = real;
        for &(a, b) in &out { st = apply(&st, amap[a], amap[b]); }
        while height(st[1]) > 0 {
            let col = top(st[1]);
            let dst = amap.iter().copied().find(|&i| height(st[i]) > 0 && ball(st[i], 0) == col).unwrap();
            st = apply(&st, 1, dst);
            full.push((1, dst));
        }
        out = full;
    }
    if r1 {
        let mut l = start[n];
        while height(l) > 0 {
            let col = top(l);
            let dst = (0..n).find(|&i| height(cur[i]) > 0 && ball(cur[i], 0) == col).unwrap();
            cur[n] = pop(cur[n]);
            cur[dst] = push(cur[dst], col);
            l = pop(l);
            out.push((n, dst));
        }
    }
    let mut lanes: Vec<Vec<u8>> = vec![vec![]];
    for _ in 0..n {
        lanes.push((1..=n as u8).collect());
    }
    if start_str.is_some() {
        lanes = (0..=n).map(|i| (0..height(start[i])).map(|k| ball(start[i], k)).collect()).collect();
    }
    for &(a, b) in &out {
        assert!(a != b && !lanes[a].is_empty() && lanes[b].len() < n);
        let x = lanes[a].pop().unwrap();
        lanes[b].push(x);
    }
    let ok = lanes.iter().all(|s| s.is_empty() || (s.len() == n && s.iter().all(|&x| x == s[0])));
    let d36 = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let s: String = out.iter().flat_map(|&(a, b)| [d36[a] as char, d36[b] as char]).collect();
    println!("N={n} moves={} verified={ok} time={:.1}s", out.len(), t0.elapsed().as_secs_f64());
    println!("{s}");
}
