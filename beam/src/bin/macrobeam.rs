//! マクロ手ビーム: 1 ステップ = 1 手 / 同色の連続を運ぶ / 入るだけ注ぐ（逆順に積む）。
//! 層はステップ数で切り、評価は 100·(g + h) + λP + μD（g は実際の手数）。
//! 12 色 272 手（Docs 付録 A）が「注ぎ合い」で出たことを受けて作った。
//!
//! 使い方: macrobeam N WIDTH [--lambda X] [--mu Y] [--learned] [--wg G] [--extra E] [--window W] [--noise K] [--seed S] [--maxpour K]
use rayon::prelude::*;
use std::collections::HashSet;

const MAXL: usize = 13;
type Lane = u64; // 上位 4 ビット = 高さ、下位から 4 ビットずつ底→上の色

#[inline]
fn height(l: Lane) -> usize {
    (l >> 60) as usize
}
#[inline]
fn ball(l: Lane, k: usize) -> u8 {
    ((l >> (4 * k)) & 15) as u8
}
#[inline]
fn top(l: Lane) -> u8 {
    ball(l, height(l) - 1)
}
#[inline]
fn pop(l: Lane) -> Lane {
    let h = height(l);
    let body = l & ((1u64 << (4 * (h - 1))) - 1);
    body | (((h - 1) as u64) << 60)
}
#[inline]
fn push(l: Lane, c: u8) -> Lane {
    let h = height(l);
    let body = l & ((1u64 << 60) - 1);
    body | ((c as u64) << (4 * h)) | (((h + 1) as u64) << 60)
}


#[derive(Clone, Copy)]
struct Cfg {
    n: usize,
    lanes: usize,
    lambda: u32,
    mu: u32,
    learned: bool,
    wg: u64,
    noise: u64,
    seed: u64,
    maxpour: usize,
}
type State = [Lane; MAXL];

fn canon(s: &State, c: &Cfg) -> State {
    let mut t = *s;
    t[..c.lanes].sort_unstable();
    t
}
fn hash(s: &State, c: &Cfg) -> u64 {
    let mut h: u64 = 0x9e3779b97f4a7c15 ^ c.seed.wrapping_mul(0x2545F4914F6CDD1D);
    for &l in &s[..c.lanes] {
        h = (h ^ l).wrapping_mul(0x100000001b3).rotate_left(29);
        h ^= h >> 31;
    }
    h
}
fn key(s: &State, c: &Cfg) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &l in &s[..c.lanes] {
        h ^= l;
        h = h.wrapping_mul(0x100000001b3);
        h ^= h >> 29;
    }
    h
}

/// 残り手数の下界 h と、初期の並び 1,2,3,… が残る分 P。
fn eval(s: &State, c: &Cfg) -> (u32, u32) {
    let mut h = 0u32;
    let mut sum_r = [0u32; 16];
    let mut max_r = [0u32; 16];
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
    let mut dl: Vec<(u8, u8, [u8; 16])> = Vec::new(); // (底の色, 長さ, 表 before keeper 決定) → 後で選ぶ
    let mut raw: Vec<(u8, usize, [u8; 16], u8)> = Vec::new(); // (底の色, r, 汚れ部分の色の個数, 汚れの個数)
    let mut uni = [false; 16];
    for &l in &s[..c.lanes] {
        let ht = height(l);
        if ht == 0 { base += 1; continue; }
        let c0 = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c0 { r += 1; }
        if r == ht { base += 1; uni[c0 as usize] = true; continue; }
        let mut cnt = [0u8; 16];
        for k in r..ht { cnt[ball(l, k) as usize] += 1; }
        raw.push((c0, r, cnt, (ht - r) as u8));
    }
    let _ = &mut dl;
    if raw.is_empty() { return 0; }
    // 残すレーン: 色ごとに r 最大の 1 本（単色レーンがあれば残さない）
    let mut keeper = [usize::MAX; 16];
    for (i, x) in raw.iter().enumerate() {
        if uni[x.0 as usize] { continue; }
        let k = &mut keeper[x.0 as usize];
        if *k == usize::MAX || raw[*k].1 < x.1 { *k = i; }
    }
    let tabs: Vec<[u8; 16]> = raw.iter().enumerate().map(|(i, x)| {
        let mut cnt = x.2;
        let mut total = x.3;
        if keeper[x.0 as usize] != i { cnt[x.0 as usize] += x.1 as u8; total += x.1 as u8; }
        let mut v: Vec<u8> = cnt[1..=n].iter().copied().filter(|&y| y > 0).collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        let mut t = [0u8; 16];
        let mut acc = 0u8;
        for m in 0..16 {
            if m > 0 && m - 1 < v.len() { acc += v[m - 1]; }
            t[m] = total.saturating_sub(acc);
        }
        t
    }).collect();
    let d = tabs.len();
    if d <= 8 {
        let mut dp = vec![0u32; 1 << d];
        for mask in 1usize..(1 << d) {
            let m = (base + mask.count_ones() as usize - 1).min(15);
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
        let last = (base + d - 1).min(15);
        let mut sum = 0u32;
        for k in 0..d {
            let m = (base + k).min(15);
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

fn learned_extra(s: &State, c: &Cfg) -> u32 {
    let mut done = 0u32;
    let mut seen = [0u8; 16];
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

fn solved(s: &State, c: &Cfg) -> bool {
    s[..c.lanes].iter().all(|&l| {
        let ht = height(l);
        ht == 0 || (ht == c.n && (0..ht).all(|k| ball(l, k) == ball(l, 0)))
    })
}

fn apply(s: &State, a: usize, b: usize, k: usize) -> State {
    let mut t = *s;
    for _ in 0..k {
        let c = top(t[a]);
        t[a] = pop(t[a]);
        t[b] = push(t[b], c);
    }
    t
}

/// マクロ手 (a, b, k)。k=1 の普通の手、同色の連続 k 個、入るだけ注ぐ k 個。
fn moves(s: &State, c: &Cfg, out: &mut Vec<(u8, u8, u8)>) {
    out.clear();
    let first_empty = (0..c.lanes).find(|&i| height(s[i]) == 0);
    for a in 0..c.lanes {
        let ha = height(s[a]);
        if ha == 0 { continue; }
        let ca = top(s[a]);
        let mut run = 1;
        while run < ha && ball(s[a], ha - 1 - run) == ca { run += 1; }
        for b in 0..c.lanes {
            if a == b { continue; }
            let hb = height(s[b]);
            if hb >= c.n { continue; }
            if hb == 0 && (Some(b) != first_empty || ha == 1) { continue; }
            let space = c.n - hb;
            out.push((a as u8, b as u8, 1));
            let r = run.min(space);
            if r >= 2 && !(hb == 0 && r == ha) { out.push((a as u8, b as u8, r as u8)); }
            let p = ha.min(space).min(c.maxpour);
            if p > r && p >= 2 && !(hb == 0 && p == ha) { out.push((a as u8, b as u8, p as u8)); }
        }
    }
}

fn initial(n: usize) -> State {
    let mut s = [0u64; MAXL];
    for i in 1..=n {
        for col in 1..=n { s[i] = push(s[i], col as u8); }
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args[1].parse().unwrap();
    let width: usize = args[2].parse().unwrap();
    let (mut lambda, mut mu, mut learned, mut wg, mut extra, mut window, mut noise, mut seed, mut maxpour) =
        (100u32, 0u32, false, 100u64, 20usize, 6usize, 0u64, 1u64, 99usize);
    let mut i = 3;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--lambda" => { lambda = v.parse().unwrap(); i += 1; }
            "--mu" => { mu = v.parse().unwrap(); i += 1; }
            "--wg" => { wg = v.parse().unwrap(); i += 1; }
            "--extra" => { extra = v.parse().unwrap(); i += 1; }
            "--window" => { window = v.parse().unwrap(); i += 1; }
            "--noise" => { noise = v.parse().unwrap(); i += 1; }
            "--seed" => { seed = v.parse().unwrap(); i += 1; }
            "--maxpour" => { maxpour = v.parse().unwrap(); i += 1; }
            "--learned" => learned = true,
            a => panic!("unknown arg {a}"),
        }
        i += 1;
    }
    let cfg = Cfg { n, lanes: n + 1, lambda, mu, learned, wg, noise, seed, maxpour };
    let t0 = std::time::Instant::now();
    let start = initial(n);
    let s0 = canon(&start, &cfg);
    let mut layer: Vec<(State, u16)> = vec![(s0, 0)];
    let mut hist: Vec<Vec<(u32, u8, u8, u8)>> = vec![vec![(0, 0, 0, 0)]];
    let mut recent: std::collections::VecDeque<HashSet<u64>> = std::collections::VecDeque::new();
    recent.push_back([key(&s0, &cfg)].into_iter().collect());
    let mut best: Option<(u16, usize, usize)> = None; // (手数, 層, 位置)
    let mut stop_at = usize::MAX;
    for depth in 1..=(10 * n * n) {
        if depth > stop_at { break; }
        let mut cand: Vec<(u64, u64, u32, u8, u8, u8, u16)> = layer
            .par_iter()
            .enumerate()
            .fold(Vec::new, |mut acc, (pi, (s, g))| {
                let mut mv = Vec::with_capacity(400);
                moves(s, &cfg, &mut mv);
                for &(a, b, k) in &mv {
                    let t = canon(&apply(s, a as usize, b as usize, k as usize), &cfg);
                    let kk = key(&t, &cfg);
                    if recent.iter().any(|h| h.contains(&kk)) { continue; }
                    let g2 = g + k as u16;
                    let (h, p) = eval(&t, &cfg);
                    let dd = if cfg.mu > 0 { d2(&t, &cfg) } else { 0 };
                    let ex = if cfg.learned { learned_extra(&t, &cfg) } else { 0 };
                    let score = g2 as u64 * cfg.wg + h as u64 * 100 + p as u64 * cfg.lambda as u64 + dd as u64 * cfg.mu as u64 + ex as u64;
                    let nz = if cfg.noise > 0 { hash(&t, &cfg) % cfg.noise } else { 0 };
                    acc.push(((score << 20) + nz.min((1 << 20) - 1), kk, pi as u32, a, b, k, g2));
                }
                acc
            })
            .reduce(Vec::new, |mut a, mut b| { a.append(&mut b); a });
        // 同じ状態は手数の少ない方を残す
        cand.par_sort_unstable_by_key(|x| (x.1, x.6, x.0));
        cand.dedup_by_key(|x| x.1);
        if cand.is_empty() { break; }
        if cand.len() > width {
            cand.select_nth_unstable_by_key(width - 1, |x| x.0);
            cand.truncate(width);
        }
        cand.par_sort_unstable_by_key(|x| x.0);
        let next: Vec<(State, u16)> = cand
            .par_iter()
            .map(|x| (canon(&apply(&layer[x.2 as usize].0, x.3 as usize, x.4 as usize, x.5 as usize), &cfg), x.6))
            .collect();
        recent.push_back(cand.iter().map(|x| x.1).collect());
        while recent.len() > window { recent.pop_front(); }
        hist.push(cand.iter().map(|x| (x.2, x.3, x.4, x.5)).collect());
        layer = next;
        for (pos, (s, g)) in layer.iter().enumerate() {
            if solved(s, &cfg) && best.map_or(true, |b| *g < b.0) {
                best = Some((*g, depth, pos));
                eprintln!("depth {depth}: {} 手の解", g);
                if stop_at == usize::MAX { stop_at = depth + extra; }
            }
        }
        // 解が出た後は、残りの最小見積もりが解の手数以上なら打ち切る
        if let Some(b) = best {
            let mn = layer.iter().map(|(s, g)| *g as u32 + eval(s, &cfg).0).min().unwrap();
            if mn >= b.0 as u32 { break; }
        }
        if depth % 10 == 0 {
            let (bh, _) = eval(&layer[0].0, &cfg);
            eprintln!("depth {depth} kept {} g={} h={bh} {:.1}s", layer.len(), layer[0].1, t0.elapsed().as_secs_f64());
        }
    }
    let Some((_, bd, mut pos)) = best else { println!("NOT FOUND"); return; };
    let mut cmoves = Vec::new();
    for d in (1..=bd).rev() {
        let (p, a, b, k) = hist[d][pos];
        cmoves.push((a, b, k));
        pos = p as usize;
    }
    cmoves.reverse();
    let mut canon_cur = s0;
    let mut cur = start;
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut mv = Vec::new();
    for &(a, b, k) in &cmoves {
        let next = canon(&apply(&canon_cur, a as usize, b as usize, k as usize), &cfg);
        moves(&cur, &cfg, &mut mv);
        let &(ra, rb, rk) = mv.iter()
            .find(|&&(x, y, z)| z == k && canon(&apply(&cur, x as usize, y as usize, z as usize), &cfg)[..cfg.lanes] == next[..cfg.lanes])
            .expect("手の復元に失敗");
        cur = apply(&cur, ra as usize, rb as usize, rk as usize);
        canon_cur = next;
        for _ in 0..rk { out.push((ra as usize, rb as usize)); }
    }
    let mut lanes: Vec<Vec<u8>> = vec![vec![]];
    for _ in 0..n { lanes.push((1..=n as u8).collect()); }
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
