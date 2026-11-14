//! 色そろえパズル（N 色・各 N 個・N+1 レーン・容量 N）の上界探し。
//! 層ごとのビーム探索。評価値は h + λP（Docs 4・5 節）、レーンの並べ替えで状態を同一視する。
//!
//! 使い方: colorsort N WIDTH [--lambda X] [--seed S] [--r1] [--r2] [--noise K] [--max-depth D]
//! 出力: 見つかった手数と、36 進 2 文字/手の文字列（レーン 0 が最初の空レーン）。
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
    lanes: usize, // 動かすレーン数（R1 なら N、そうでなければ N+1）
    r2: bool,
    lambda: u32,
    noise: u64,
    seed: u64,
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

fn solved(s: &State, c: &Cfg) -> bool {
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
    let mut s = [0u64; MAXL];
    for i in 1..=n {
        let mut l = 0;
        for col in 1..=n {
            l = push(l, col as u8);
        }
        s[i] = l;
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args[1].parse().unwrap();
    let width: usize = args[2].parse().unwrap();
    let mut lambda = 100u32; // P の重み（%）
    let mut seed = 1u64;
    let mut r1 = false;
    let mut r2 = false;
    let mut noise = 0u64;
    let mut max_depth = 10 * n * n;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--lambda" => { lambda = args[i + 1].parse().unwrap(); i += 1; }
            "--seed" => { seed = args[i + 1].parse().unwrap(); i += 1; }
            "--noise" => { noise = args[i + 1].parse().unwrap(); i += 1; }
            "--max-depth" => { max_depth = args[i + 1].parse().unwrap(); i += 1; }
            "--r1" => r1 = true,
            "--r2" => r2 = true,
            a => panic!("unknown arg {a}"),
        }
        i += 1;
    }
    assert!((2..=12).contains(&n));
    let cfg = Cfg { n, lanes: if r1 { n } else { n + 1 }, r2, lambda, noise, seed };
    let t0 = std::time::Instant::now();

    // R1 ではレーン N（最後の満杯レーン）を動かさない。動かすのはレーン 0..N-1。
    let start = initial(n);
    let mut layer: Vec<State> = vec![canon(&start, &cfg)];
    let mut history: Vec<Vec<State>> = vec![layer.clone()];
    let mut parents: Vec<Vec<u32>> = vec![vec![0]];
    let mut seen: HashSet<u64> = HashSet::new();
    seen.insert(key(&layer[0], &cfg));
    let frozen = if r1 { start[n] } else { 0 };

    let mut found: Option<(usize, usize)> = None;
    for depth in 1..=max_depth {
        let cand: Vec<(u64, u64, u32, State)> = layer
            .par_iter()
            .enumerate()
            .fold(Vec::new, |mut acc, (pi, s)| {
                let mut mv = Vec::with_capacity(160);
                moves(s, &cfg, &mut mv);
                for &(a, b) in &mv {
                    let t = canon(&apply(s, a as usize, b as usize), &cfg);
                    let (h, p) = eval(&t, &cfg);
                    let score = (h as u64) * 100 + (p as u64) * cfg.lambda as u64;
                    let nz = if cfg.noise > 0 { hash(&t, &cfg) % cfg.noise } else { 0 };
                    let k = key(&t, &cfg);
                    acc.push(((score << 20) + nz.min((1 << 20) - 1), k, pi as u32, t));
                }
                acc
            })
            .reduce(Vec::new, |mut a, mut b| { a.append(&mut b); a });
        // 重複を除き、過去に保存した状態も除く
        let mut cand = cand;
        cand.par_sort_unstable_by_key(|x| (x.1, x.0));
        cand.dedup_by_key(|x| x.1);
        let mut cand: Vec<_> = cand.into_iter().filter(|x| !seen.contains(&x.1)).collect();
        if cand.is_empty() {
            eprintln!("depth {depth}: 候補なし");
            break;
        }
        if cand.len() > width {
            cand.select_nth_unstable_by_key(width - 1, |x| x.0);
            cand.truncate(width);
        }
        cand.par_sort_unstable_by_key(|x| x.0);
        for x in &cand {
            seen.insert(x.1);
        }
        let best = &cand[0];
        let (bh, bp) = eval(&best.3, &cfg);
        if depth % 10 == 0 {
            eprintln!("depth {depth} kept {} best h={bh} P={bp} seen={} {:.1}s", cand.len(), seen.len(), t0.elapsed().as_secs_f64());
        }
        layer = cand.iter().map(|x| x.3).collect();
        parents.push(cand.iter().map(|x| x.2).collect());
        history.push(layer.clone());
        if let Some(pos) = layer.iter().position(|s| solved(s, &cfg)) {
            found = Some((depth, pos));
            break;
        }
    }
    let Some((depth, mut pos)) = found else {
        println!("NOT FOUND");
        return;
    };
    // 正規形の列を逆にたどる
    let mut chain = vec![history[depth][pos]];
    for d in (1..=depth).rev() {
        pos = parents[d][pos] as usize;
        chain.push(history[d - 1][pos]);
    }
    chain.reverse();
    // 実際のレーン番号で手を復元する
    let mut cur = start;
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut mv = Vec::new();
    for next in &chain[1..] {
        moves(&cur, &cfg, &mut mv);
        let &(a, b) = mv
            .iter()
            .find(|&&(a, b)| canon(&apply(&cur, a as usize, b as usize), &cfg)[..cfg.lanes] == next[..cfg.lanes])
            .expect("手の復元に失敗");
        cur = apply(&cur, a as usize, b as usize);
        out.push((a as usize, b as usize));
    }
    if r1 {
        // 終盤: レーン N を上から配る
        let mut l = frozen;
        while height(l) > 0 {
            let col = top(l);
            let dst = (0..n).find(|&i| height(cur[i]) > 0 && ball(cur[i], 0) == col).unwrap();
            cur[n] = pop(cur[n]);
            cur[dst] = push(cur[dst], col);
            l = pop(l);
            out.push((n, dst));
        }
    }
    // 検証（Docs 7 節と同じ条件）
    let mut lanes: Vec<Vec<u8>> = vec![vec![]];
    for _ in 0..n {
        lanes.push((1..=n as u8).collect());
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
