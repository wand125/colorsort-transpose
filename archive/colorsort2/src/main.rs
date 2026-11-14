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

/// 手の一覧に、運ぶ個数 k を付ける。pour なら同じ色を運べるだけ運ぶ手（k ≥ 1）、
/// pour_also なら 1 個ずつの手も加える。
fn moves_k(s: &State, c: &Cfg, pour: bool, pour_also: bool, out: &mut Vec<(u8, u8, u8)>) {
    let mut mv = Vec::with_capacity(160);
    moves(s, c, &mut mv);
    out.clear();
    for &(a, b) in &mv {
        if !pour {
            out.push((a, b, 1));
            continue;
        }
        let la = s[a as usize];
        let ha = height(la);
        let col = top(la);
        let mut run = 1;
        while run < ha && ball(la, ha - 1 - run) == col {
            run += 1;
        }
        let space = c.n - height(s[b as usize]);
        let mut k = run.min(space);
        // 空レーンへ全部を移すだけの手は形が変わらない
        if height(s[b as usize]) == 0 && k == ha {
            k -= 1;
        }
        if k >= 1 {
            out.push((a, b, k as u8));
        }
        if pour_also && k > 1 {
            out.push((a, b, 1));
        }
    }
}

fn apply_k(s: &State, a: usize, b: usize, k: usize) -> State {
    let mut t = *s;
    for _ in 0..k {
        t = apply(&t, a, b);
    }
    t
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args[1].parse().unwrap();
    let width: usize = args[2].parse().unwrap();
    let mut lambda = 100u32; // P の重み（%）
    let mut seed = 1u64;
    let (mut r1, mut r2, mut pour, mut pour_also) = (false, false, false, false);
    let mut noise = 0u64;
    let mut max_cost = 10 * n * n;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--lambda" => { lambda = args[i + 1].parse().unwrap(); i += 1; }
            "--seed" => { seed = args[i + 1].parse().unwrap(); i += 1; }
            "--noise" => { noise = args[i + 1].parse().unwrap(); i += 1; }
            "--max-cost" => { max_cost = args[i + 1].parse().unwrap(); i += 1; }
            "--r1" => r1 = true,
            "--r2" => r2 = true,
            "--pour" => pour = true,
            "--pour-also" => { pour = true; pour_also = true; }
            a => panic!("unknown arg {a}"),
        }
        i += 1;
    }
    assert!((2..=12).contains(&n));
    let cfg = Cfg { n, lanes: if r1 { n } else { n + 1 }, r2, lambda, noise, seed };
    let t0 = std::time::Instant::now();

    // R1 ではレーン N（最後の満杯レーン）を動かさない。動かすのはレーン 0..N-1。
    let start = initial(n);
    let frozen = if r1 { start[n] } else { 0 };
    // 手数（コスト）ごとのバケット。候補は (評価, 鍵, 親の (コスト, 位置), 状態)
    type Cand = (u64, u64, (u32, u32), State);
    let mut pending: Vec<Vec<Cand>> = vec![Vec::new(); max_cost + 16];
    let mut kept: Vec<Vec<State>> = vec![Vec::new(); max_cost + 16];
    let mut parent: Vec<Vec<(u32, u32)>> = vec![Vec::new(); max_cost + 16];
    let s0 = canon(&start, &cfg);
    kept[0].push(s0);
    parent[0].push((0, 0));
    let mut seen: HashSet<u64> = HashSet::new();
    seen.insert(key(&s0, &cfg));
    let mut found: Option<(usize, usize)> = None;

    for g in 0..=max_cost {
        if g > 0 {
            let mut cand = std::mem::take(&mut pending[g]);
            if cand.is_empty() {
                continue;
            }
            cand.par_sort_unstable_by_key(|x| (x.1, x.0));
            cand.dedup_by_key(|x| x.1);
            let mut cand: Vec<_> = cand.into_iter().filter(|x| !seen.contains(&x.1)).collect();
            if cand.len() > width {
                cand.select_nth_unstable_by_key(width - 1, |x| x.0);
                cand.truncate(width);
            }
            cand.par_sort_unstable_by_key(|x| x.0);
            for x in &cand {
                seen.insert(x.1);
            }
            kept[g] = cand.iter().map(|x| x.3).collect();
            parent[g] = cand.iter().map(|x| x.2).collect();
            if g % 10 == 0 && !kept[g].is_empty() {
                let (bh, bp) = eval(&kept[g][0], &cfg);
                eprintln!("cost {g} kept {} best h={bh} P={bp} seen={} {:.1}s", kept[g].len(), seen.len(), t0.elapsed().as_secs_f64());
            }
            if let Some(pos) = kept[g].iter().position(|s| solved(s, &cfg)) {
                found = Some((g, pos));
                break;
            }
        }
        if kept[g].is_empty() {
            continue;
        }
        let layer = &kept[g];
        let exp: Vec<(usize, Cand)> = layer
            .par_iter()
            .enumerate()
            .fold(Vec::new, |mut acc, (pi, s)| {
                let mut mv = Vec::with_capacity(320);
                moves_k(s, &cfg, pour, pour_also, &mut mv);
                for &(a, b, k) in &mv {
                    let t = canon(&apply_k(s, a as usize, b as usize, k as usize), &cfg);
                    let (h, p) = eval(&t, &cfg);
                    let score = (h as u64) * 100 + (p as u64) * cfg.lambda as u64;
                    let nz = if cfg.noise > 0 { hash(&t, &cfg) % cfg.noise } else { 0 };
                    acc.push((g + k as usize, ((score << 20) + nz.min((1 << 20) - 1), key(&t, &cfg), (g as u32, pi as u32), t)));
                }
                acc
            })
            .reduce(Vec::new, |mut a, mut b| { a.append(&mut b); a });
        for (gk, c) in exp {
            if gk <= max_cost {
                pending[gk].push(c);
            }
        }
        // 候補が多すぎるバケットは途中で絞る（メモリ対策）
        for q in pending.iter_mut().skip(g + 1).take(n + 1) {
            if q.len() > 8 * width {
                q.select_nth_unstable_by_key(4 * width, |x| x.0);
                q.truncate(4 * width);
            }
        }
    }
    let Some((cost, mut pos)) = found else {
        println!("NOT FOUND");
        return;
    };
    // 正規形の列を逆にたどる
    let mut chain = vec![kept[cost][pos]];
    let mut g = cost;
    while g > 0 {
        let (pg, pp) = parent[g][pos];
        g = pg as usize;
        pos = pp as usize;
        chain.push(kept[g][pos]);
    }
    chain.reverse();
    // 実際のレーン番号で手を復元する
    let mut cur = start;
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut mv = Vec::new();
    for next in &chain[1..] {
        moves_k(&cur, &cfg, pour, pour_also, &mut mv);
        let &(a, b, k) = mv
            .iter()
            .find(|&&(a, b, k)| canon(&apply_k(&cur, a as usize, b as usize, k as usize), &cfg)[..cfg.lanes] == next[..cfg.lanes])
            .expect("手の復元に失敗");
        cur = apply_k(&cur, a as usize, b as usize, k as usize);
        for _ in 0..k {
            out.push((a as usize, b as usize));
        }
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
