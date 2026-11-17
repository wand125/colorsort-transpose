//! Layer beam search between lane multisets. JSON permutation[target] = endpoint.
use rayon::prelude::*;
use std::collections::HashSet;
use std::time::Instant;
type Lane = u128;
type State = Vec<Lane>;
const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const BODY: u128 = (1u128 << 120) - 1;
fn height(l: Lane) -> usize {
    (l >> 120) as usize
}
fn push(l: Lane, c: u8) -> Lane {
    (l & BODY) | ((c as u128) << (5 * height(l))) | (((height(l) + 1) as u128) << 120)
}
fn apply(s: &[Lane], a: usize, b: usize) -> State {
    let mut t = s.to_vec();
    let h = height(t[a]);
    let c = ((t[a] >> (5 * (h - 1))) & 31) as u8;
    t[a] = (t[a] & ((1u128 << (5 * (h - 1))) - 1)) | (((h - 1) as u128) << 120);
    t[b] = push(t[b], c);
    t
}
fn canon(mut s: State) -> State {
    s.sort_unstable();
    s
}
fn lcp(a: Lane, b: Lane) -> usize {
    let x = (a ^ b) & BODY;
    let k = if x == 0 {
        24
    } else {
        x.trailing_zeros() as usize / 5
    };
    k.min(height(a)).min(height(b))
}
// Integer Hungarian minimum cost matching, including empty and repeated lanes.
fn hungarian(c: &[[i32; 25]; 25], n: usize) -> usize {
    let (mut u, mut v, mut p, mut way) = ([0; 26], [0; 26], [0usize; 26], [0usize; 26]);
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0;
        let mut minv = [i32::MAX; 26];
        let mut used = [false; 26];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let (mut delta, mut j1) = (i32::MAX, 0);
            for j in 1..=n {
                if !used[j] {
                    let cur = c[i0 - 1][j - 1] - u[i0] - v[j];
                    if cur < minv[j] {
                        minv[j] = cur;
                        way[j] = j0;
                    }
                    if minv[j] < delta {
                        delta = minv[j];
                        j1 = j;
                    }
                }
            }
            for j in 0..=n {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else if j > 0 {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    (-v[0]) as usize
}
fn lower_bound(s: &[Lane], t: &[Lane], budget: usize) -> usize {
    // Equal lanes can be matched to one another by the prefix exchange inequality.
    let (mut a, mut b) = ([0; 25], [0; 25]);
    let (mut i, mut j, mut na, mut nb) = (0, 0, 0, 0);
    while i < s.len() || j < t.len() {
        if i < s.len() && j < t.len() && s[i] == t[j] {
            i += 1;
            j += 1;
        } else if j == t.len() || (i < s.len() && s[i] < t[j]) {
            a[na] = s[i];
            na += 1;
            i += 1;
        } else {
            b[nb] = t[j];
            nb += 1;
            j += 1;
        }
    }
    debug_assert_eq!(na, nb);
    if na == 0 {
        return 0;
    }
    let mut c = [[0; 25]; 25];
    let mut cheap = 0;
    for i in 0..na {
        let mut best = usize::MAX;
        for j in 0..na {
            let cost = height(a[i]) - lcp(a[i], b[j]);
            c[i][j] = cost as i32;
            best = best.min(cost);
        }
        cheap += best;
    }
    if cheap > budget {
        return cheap;
    }
    hungarian(&c, na)
}
fn key(s: &[Lane]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &l in s {
        h = (h ^ l as u64).wrapping_mul(0x100000001b3).rotate_left(23);
        h = (h ^ (l >> 64) as u64).wrapping_mul(0x100000001b3);
        h ^= h >> 29;
    }
    h
}
#[derive(Clone, Copy, Debug)]
struct Edge {
    parent: u32,
    a: u8,
    b: u8,
}
#[derive(Clone, Copy)]
struct Candidate {
    hash: u64,
    edge: Edge,
    h: u16,
}
fn next_state(c: &Candidate, layer: &[State]) -> State {
    canon(apply(
        &layer[c.edge.parent as usize],
        c.edge.a as usize,
        c.edge.b as usize,
    ))
}
struct Search {
    path: Option<Vec<Edge>>,
    generated: u64,
    evaluated: u64,
    truncated: bool,
    depth: usize,
    h0: usize,
    peak_candidates: usize,
}
fn search(n: usize, start: State, target: State, max: usize, width: usize) -> Search {
    let bound = max - 1;
    let h0 = lower_bound(&start, &target, usize::MAX);
    let mut r = Search {
        path: None,
        generated: 0,
        evaluated: 0,
        truncated: false,
        depth: 0,
        h0,
        peak_candidates: 0,
    };
    if h0 > bound {
        return r;
    }
    let mut layer = vec![start];
    let mut hist: Vec<Vec<Edge>> = vec![];
    let mut recent: HashSet<State> = layer.iter().cloned().collect();
    let mut previous = HashSet::new();
    if layer[0] == target {
        r.path = Some(vec![]);
        return r;
    }
    for depth in 1..=bound {
        let pieces: Vec<_> = layer
            .par_iter()
            .enumerate()
            .map(|(pi, s)| {
                let mut out = Vec::new();
                let (mut generated, mut evaluated) = (0u64, 0u64);
                for a in 0..s.len() {
                    if height(s[a]) == 0 || (a > 0 && s[a] == s[a - 1]) {
                        continue;
                    }
                    for b in 0..s.len() {
                        if a == b || height(s[b]) == n {
                            continue;
                        }
                        if b > 0 && s[b] == s[b - 1] && b - 1 != a {
                            continue;
                        }
                        if height(s[a]) == 1 && s[b] == 0 {
                            continue;
                        }
                        generated += 1;
                        let t = canon(apply(s, a, b));
                        if recent.contains(&t) || previous.contains(&t) {
                            continue;
                        }
                        evaluated += 1;
                        let h = lower_bound(&t, &target, bound - depth);
                        if depth + h > bound {
                            continue;
                        }
                        out.push(Candidate {
                            hash: key(&t),
                            edge: Edge {
                                parent: pi as u32,
                                a: a as u8,
                                b: b as u8,
                            },
                            h: h as u16,
                        });
                    }
                }
                (out, generated, evaluated)
            })
            .collect();
        let mut cand = Vec::new();
        for (mut part, g, e) in pieces {
            cand.append(&mut part);
            r.generated += g;
            r.evaluated += e;
        }
        r.depth = depth;
        r.peak_candidates = r.peak_candidates.max(cand.len());
        // Hash only groups states; exact comparisons preserve hash collisions.
        cand.par_sort_unstable_by_key(|c| (c.hash, c.edge.parent, c.edge.a, c.edge.b));
        let mut unique: Vec<Candidate> = Vec::with_capacity(cand.len());
        let mut group = 0;
        for c in cand {
            if unique.last().is_none_or(|last| last.hash != c.hash) {
                group = unique.len();
            }
            if unique[group..]
                .iter()
                .any(|p| next_state(p, &layer) == next_state(&c, &layer))
            {
                continue;
            }
            unique.push(c);
        }
        let mut cand = unique;
        if cand.is_empty() {
            break;
        }
        // g is constant within a layer: rank by g + lambda*h, lambda=1.
        // Hash is only a deterministic tie-breaker independent of lane names.
        if cand.len() > width {
            r.truncated = true;
            cand.select_nth_unstable_by_key(width - 1, |c| (c.h, c.hash));
            cand.truncate(width);
        }
        cand.par_sort_unstable_by_key(|c| (c.h, c.hash));
        let next: Vec<State> = cand.par_iter().map(|c| next_state(c, &layer)).collect();
        hist.push(cand.iter().map(|c| c.edge).collect());
        if let Some(mut pos) = next.iter().position(|s| *s == target) {
            let mut path = Vec::new();
            for edges in hist.iter().rev() {
                let e = edges[pos];
                path.push(e);
                pos = e.parent as usize;
            }
            path.reverse();
            r.path = Some(path);
            break;
        }
        previous = recent;
        recent = next.iter().cloned().collect();
        layer = next;
        eprintln!(
            "depth={depth} kept={} best_h={} generated={}",
            layer.len(),
            cand[0].h,
            r.generated
        );
    }
    r
}
fn parse(n: usize, text: &str) -> Result<State, String> {
    let mut counts = [0usize; 25];
    let mut s = Vec::new();
    for word in text.split(',') {
        let mut l = 0;
        if word != "." {
            if word.is_empty() || word.len() > n {
                return Err("use . for empty; lane exceeds capacity".into());
            }
            for ch in word.chars() {
                let c = ch.to_digit(36).ok_or("invalid base36 color")? as usize;
                if c == 0 || c > n {
                    return Err("color outside 1..N".into());
                }
                counts[c] += 1;
                l = push(l, c as u8);
            }
        }
        s.push(l);
    }
    if s.len() != n + 1 || counts[1..=n].iter().any(|&x| x != n) {
        return Err("need N+1 lanes and N copies of each color".into());
    }
    Ok(s)
}
fn restore(start: State, target: &[Lane], path: &[Edge]) -> (String, Vec<usize>) {
    let mut real = start;
    let mut out = String::new();
    for e in path {
        let mut order: Vec<_> = (0..real.len()).collect();
        order.sort_unstable_by_key(|&i| (real[i], i));
        let (a, b) = (order[e.a as usize], order[e.b as usize]);
        assert!(a != b && height(real[a]) > 0 && height(real[b]) < real.len() - 1);
        real = apply(&real, a, b);
        out.push(D[a] as char);
        out.push(D[b] as char);
    }
    let mut used = vec![false; real.len()];
    let p = target
        .iter()
        .map(|lane| {
            let i = (0..real.len())
                .find(|&i| !used[i] && real[i] == *lane)
                .expect("endpoint differs from target");
            used[i] = true;
            i
        })
        .collect();
    (out, p)
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!("wbeam N --from LANES --to LANES --max L --width W [--threads T]\nLANES: comma-separated bottom-to-top base36, . for empty.\nSearches <= L-1 moves. JSON permutation[target lane]=endpoint lane.\nFOUND is shortest depth found by this beam; truncation prevents a global optimality claim.");
        return Ok(());
    }
    let n: usize = args
        .first()
        .ok_or("missing N")?
        .parse()
        .map_err(|_| "invalid N")?;
    if !(1..=24).contains(&n) {
        return Err("N must be in 1..24".into());
    }
    let (mut from, mut to, mut max, mut width, mut threads) = (None, None, None, None, 1usize);
    let mut i = 1;
    while i < args.len() {
        let value = args.get(i + 1).ok_or("missing option value")?;
        match args[i].as_str() {
            "--from" => {
                if from.is_some() {
                    return Err("duplicate --from".into());
                }
                from = Some(parse(n, value)?);
            }
            "--to" => {
                if to.is_some() {
                    return Err("duplicate --to".into());
                }
                to = Some(parse(n, value)?);
            }
            "--max" => max = Some(value.parse::<usize>().map_err(|_| "invalid --max")?),
            "--width" => width = Some(value.parse::<usize>().map_err(|_| "invalid --width")?),
            "--threads" => threads = value.parse().map_err(|_| "invalid --threads")?,
            _ => return Err(format!("unknown option {}", args[i])),
        }
        i += 2;
    }
    let (start, target) = (from.ok_or("missing --from")?, to.ok_or("missing --to")?);
    let (max, width) = (max.ok_or("missing --max")?, width.ok_or("missing --width")?);
    if max == 0 || width == 0 || width > u32::MAX as usize || !(1..=4).contains(&threads) {
        return Err("require L,W>0, W<=u32::MAX, threads in 1..4".into());
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .map_err(|e| e.to_string())?;
    let begin = Instant::now();
    let r = search(n, canon(start.clone()), canon(target.clone()), max, width);
    let (status, moves, p) = match &r.path {
        Some(path) => {
            let (s, p) = restore(start, &target, path);
            ("FOUND", s, p)
        }
        None => ("NOT_FOUND", String::new(), Vec::new()),
    };
    println!("{{\"status\":\"{status}\",\"moves\":\"{moves}\",\"length\":{},\"permutation\":{:?},\"h0\":{},\"depth\":{},\"truncated\":{},\"generated\":{},\"evaluated\":{},\"peak_candidates\":{},\"seconds\":{:.6},\"threads\":{threads}}}",moves.len()/2,p,r.h0,r.depth,r.truncated,r.generated,r.evaluated,r.peak_candidates,begin.elapsed().as_secs_f64());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("wbeam: {e}");
        std::process::exit(2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn perm(a: &mut [usize], i: usize, c: &[[i32; 25]; 25], best: &mut usize) {
        if i == a.len() {
            *best = (*best).min(a.iter().enumerate().map(|(i, &j)| c[i][j] as usize).sum());
            return;
        }
        for j in i..a.len() {
            a.swap(i, j);
            perm(a, i + 1, c, best);
            a.swap(i, j);
        }
    }
    #[test]
    fn assignment_matches_bruteforce() {
        for seed in 0..30 {
            let mut c = [[0; 25]; 25];
            for i in 0..6 {
                for j in 0..6 {
                    c[i][j] = ((i * 17 + j * 31 + i * j * seed + seed) % 19) as i32;
                }
            }
            let mut best = usize::MAX;
            perm(&mut [0, 1, 2, 3, 4, 5], 0, &c, &mut best);
            assert_eq!(hungarian(&c, 6), best);
        }
    }
    #[test]
    fn exhaustive_n2_distances_and_beam() {
        use std::collections::{HashMap, VecDeque};
        let initial = canon(parse(2, ".,12,12").unwrap());
        let mut all = HashSet::from([initial.clone()]);
        let mut q = VecDeque::from([initial]);
        while let Some(s) = q.pop_front() {
            for a in 0..3 {
                for b in 0..3 {
                    if a == b || height(s[a]) == 0 || height(s[b]) == 2 {
                        continue;
                    }
                    let t = canon(apply(&s, a, b));
                    if all.insert(t.clone()) {
                        q.push_back(t);
                    }
                }
            }
        }
        for target in &all {
            let mut dist = HashMap::from([(target.clone(), 0)]);
            let mut q = VecDeque::from([target.clone()]);
            while let Some(s) = q.pop_front() {
                let d = dist[&s];
                for a in 0..3 {
                    for b in 0..3 {
                        if a == b || height(s[a]) == 0 || height(s[b]) == 2 {
                            continue;
                        }
                        let t = canon(apply(&s, a, b));
                        if !dist.contains_key(&t) {
                            dist.insert(t.clone(), d + 1);
                            q.push_back(t);
                        }
                    }
                }
            }
            for s in &all {
                let d = dist[s];
                assert!(lower_bound(s, target, usize::MAX) <= d);
                let r = search(2, s.clone(), target.clone(), d + 1, 10000);
                assert_eq!(r.path.as_ref().unwrap().len(), d);
                restore(s.clone(), target, r.path.as_ref().unwrap());
                if d > 0 {
                    assert!(search(2, s.clone(), target.clone(), d, 10000)
                        .path
                        .is_none());
                }
            }
        }
    }
}
