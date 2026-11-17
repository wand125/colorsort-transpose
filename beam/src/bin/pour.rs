//! Cost-bucket pour beam, matching colorsort_runs/pour/beamMN.cpp.
//! Boards live for N+1 cost layers; permanent history contains only parent/a/b/r.
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::time::Instant;
const MAXL: usize = 25;
type Lane = u128;
type State = [Lane; MAXL];
fn height(l: Lane) -> usize {
    (l >> 120) as usize
}
fn ball(l: Lane, k: usize) -> u8 {
    ((l >> (5 * k)) & 31) as u8
}
fn push(l: Lane, c: u8) -> Lane {
    let h = height(l);
    (l & ((1u128 << 120) - 1)) | ((c as u128) << (5 * h)) | (((h + 1) as u128) << 120)
}
fn pop(l: Lane) -> Lane {
    let h = height(l);
    (l & ((1u128 << (5 * (h - 1))) - 1)) | (((h - 1) as u128) << 120)
}
fn initial(n: usize) -> State {
    let mut s = [0; MAXL];
    for l in &mut s[1..=n] {
        for c in 1..=n {
            *l = push(*l, c as u8);
        }
    }
    s
}
fn apply(s: &mut [Lane], a: usize, b: usize, r: usize) {
    for _ in 0..r {
        let c = ball(s[a], height(s[a]) - 1);
        s[a] = pop(s[a]);
        s[b] = push(s[b], c);
    }
}
fn parse(n: usize, text: &str) -> State {
    let lanes: Vec<_> = text.split(',').collect();
    assert_eq!(lanes.len(), n + 1, "--start needs N+1 lanes");
    let mut s = [0; MAXL];
    let mut count = [0; MAXL];
    for (i, l) in lanes.iter().enumerate() {
        if *l == "." {
            continue;
        }
        assert!(!l.is_empty());
        for ch in l.chars() {
            let c = ch.to_digit(36).expect("base36 colour") as usize;
            assert!(c > 0 && c <= n && height(s[i]) < n, "invalid start");
            count[c] += 1;
            s[i] = push(s[i], c as u8);
        }
    }
    assert!(
        count[1..=n].iter().all(|&x| x == n),
        "each colour must occur N times"
    );
    s
}
fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^ (x >> 33)
}
fn lane_hash(l: Lane) -> u64 {
    let mut x = 0x9e3779b97f4a7c15u64.wrapping_add(height(l) as u64);
    for k in 0..height(l) {
        x = mix(x.wrapping_mul(31).wrapping_add(ball(l, k) as u64 + 1));
    }
    mix(x)
}
fn hash(s: &[Lane]) -> u64 {
    s.iter().fold(0u64, |h, &l| h.wrapping_add(lane_hash(l)))
}
#[derive(Default)]
struct Identity(u64);
impl Hasher for Identity {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, b: &[u8]) {
        let mut a = [0; 8];
        a[..b.len()].copy_from_slice(b);
        self.0 = u64::from_ne_bytes(a);
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = x;
    }
}
type Seen = HashSet<u64, BuildHasherDefault<Identity>>;
fn misplaced(s: &[Lane], n: usize) -> u32 {
    let mut best = [0; MAXL];
    let mut sum = 0;
    for &l in s {
        let ht = height(l);
        if ht == 0 {
            continue;
        }
        let c = ball(l, 0) as usize;
        let mut r = 1;
        while r < ht && ball(l, r) as usize == c {
            r += 1
        }
        sum += ht;
        best[c] = best[c].max(r);
    }
    (sum - best[1..=n].iter().sum::<usize>()) as u32
}
#[cfg(test)]
fn features(s: &[Lane], n: usize) -> (u32, u32, u32) {
    let mut t = 0;
    let mut present = [0u32; MAXL];
    let (mut total, mut m1, mut m2) = (0, 0, 0);
    for (i, &l) in s.iter().enumerate() {
        let h = height(l);
        for k in 0..h {
            present[ball(l, k) as usize] |= 1 << i;
            if k > 0 && ball(l, k) != ball(l, k - 1) {
                t += 1
            }
        }
        let mut p = 0;
        while p < h && ball(l, p) as usize == p + 1 {
            p += 1
        }
        total += p;
        if p > m1 {
            m2 = m1;
            m1 = p
        } else if p > m2 {
            m2 = p
        }
    }
    (
        t,
        present[1..=n].iter().map(|x| x.count_ones() - 1).sum(),
        (total - m1 - m2) as u32,
    )
}

// Minimum assignment cost, used instead of d2's exponential subset DP. Each lane
// is assigned a distinct disposal position. The minimisation is identical.
fn assignment(rows: &[[u8; MAXL]], base: usize, n: usize) -> u32 {
    let d = rows.len();
    let mut u = vec![0i32; d + 1];
    let mut v = vec![0i32; d + 1];
    let mut p = vec![0usize; d + 1];
    let mut way = vec![0usize; d + 1];
    for i in 1..=d {
        p[0] = i;
        let mut j0 = 0;
        let mut min = vec![i32::MAX; d + 1];
        let mut used = vec![false; d + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let (mut delta, mut j1) = (i32::MAX, 0);
            for j in 1..=d {
                if !used[j] {
                    let cur = rows[i0 - 1][(base + j - 1).min(n)] as i32 - u[i0] - v[j];
                    if cur < min[j] {
                        min[j] = cur;
                        way[j] = j0
                    }
                    if min[j] < delta {
                        delta = min[j];
                        j1 = j
                    }
                }
            }
            for j in 0..=d {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta
                } else {
                    min[j] -= delta
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
    (-v[0]) as u32
}
// Port of lowerbound/src/main.rs d2, widened to 24 colours. The >512
// combinations relaxation is the same admissible relaxation as the original.
fn hd(s: &[Lane], n: usize) -> u32 {
    let mut base = 0;
    let mut uniform = [false; MAXL];
    let mut dl = Vec::new();
    for &l in s {
        let ht = height(l);
        if ht == 0 {
            base += 1;
            continue;
        }
        let c = ball(l, 0) as usize;
        let mut r = 1;
        while r < ht && ball(l, r) as usize == c {
            r += 1
        }
        if r == ht {
            base += 1;
            uniform[c] = true;
            continue;
        }
        let mut cnt = [0u8; MAXL];
        for k in r..ht {
            cnt[ball(l, k) as usize] += 1
        }
        let tab = |cnt: &[u8; MAXL]| {
            let mut vals = cnt[1..=n].to_vec();
            vals.sort_unstable_by(|a, b| b.cmp(a));
            let mut out = [0u8; MAXL];
            out[0] = vals.iter().sum();
            for m in 1..=n {
                out[m] = out[m - 1] - vals[m - 1]
            }
            out
        };
        let keep = tab(&cnt);
        cnt[c] += r as u8;
        dl.push((c, keep, tab(&cnt)));
    }
    if dl.is_empty() {
        return misplaced(s, n);
    }
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, x) in dl.iter().enumerate() {
        if let Some(g) = groups.iter_mut().find(|g| g.0 == x.0) {
            g.1.push(i)
        } else {
            groups.push((x.0, vec![i]))
        }
    }
    let opts: Vec<_> = groups
        .iter()
        .map(|g| {
            let mut x = g.1.clone();
            if uniform[g.0] {
                x.push(usize::MAX)
            }
            x
        })
        .collect();
    let combos: usize = opts.iter().map(Vec::len).product();
    let relax = combos > 512;
    let mut idx = vec![0; groups.len()];
    let mut best = u32::MAX;
    loop {
        let rows: Vec<_> = dl
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if relax {
                    x.1
                } else {
                    let g = groups.iter().position(|g| g.0 == x.0).unwrap();
                    if opts[g][idx[g]] == i {
                        x.1
                    } else {
                        x.2
                    }
                }
            })
            .collect();
        best = best.min(assignment(&rows, base, n));
        if relax {
            break;
        }
        let mut j = 0;
        loop {
            if j == idx.len() {
                return misplaced(s, n) + best;
            }
            idx[j] += 1;
            if idx[j] < opts[j].len() {
                break;
            }
            idx[j] = 0;
            j += 1
        }
    }
    misplaced(s, n) + best
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Item {
    c: usize,
    nk: u32,
    keep: [u8; MAXL],
    nf: u32,
    full: [u8; MAXL],
    can: bool,
}
struct Joint {
    its: Vec<Item>,
    counts: Vec<u32>,
    step: Vec<u32>,
    done: u32,
    classes: Vec<u32>,
    n: usize,
    memo: HashMap<(u32, u32), u32>,
}
impl Joint {
    fn new(mut its: Vec<Item>, n: usize) -> Self {
        its.sort_unstable();
        let mut rows: Vec<Item> = Vec::new();
        let mut counts = Vec::new();
        for it in its {
            if rows.last() == Some(&it) {
                *counts.last_mut().unwrap() += 1
            } else {
                rows.push(it);
                counts.push(1u32)
            }
        }
        let mut step = Vec::new();
        let (mut mul, mut done) = (1u32, 0);
        for &cnt in &counts {
            step.push(mul);
            done += cnt * mul;
            mul *= cnt + 1
        }
        let mut classes = Vec::new();
        let mut used = 0u32;
        for c in 1..=n {
            if used >> c & 1 != 0 || rows.iter().any(|it| it.can && it.c == c) {
                continue;
            }
            let mut mask = 1 << c;
            for d in c + 1..=n {
                if !rows.iter().any(|it| it.can && it.c == d)
                    && rows
                        .iter()
                        .all(|it| it.keep[c] == it.keep[d] && it.full[c] == it.full[d])
                {
                    mask |= 1 << d
                }
            }
            used |= mask;
            if mask.count_ones() > 1 {
                classes.push(mask)
            }
        }
        Self {
            its: rows,
            counts,
            step,
            done,
            classes,
            n,
            memo: HashMap::new(),
        }
    }
    fn cost(&self, cnt: &[u8; MAXL], o: u32) -> u32 {
        (1..=self.n)
            .filter(|&c| o >> c & 1 == 0)
            .map(|c| cnt[c] as u32)
            .sum()
    }
    fn go(&mut self, code: u32, mut o: u32, budget: u32) -> u32 {
        if code == self.done {
            return 0;
        }
        for &cl in &self.classes {
            let k = (o & cl).count_ones();
            let mut mm = cl;
            let mut low = 0;
            for _ in 0..k {
                low |= mm & mm.wrapping_neg();
                mm &= mm - 1
            }
            o = (o & !cl) | low
        }
        let stored = *self.memo.get(&(code, o)).unwrap_or(&0);
        if stored > budget {
            return stored;
        }
        let mut live = Vec::new();
        let mut h = 0;
        let mut remain = 0;
        let mut call = [0u32; MAXL];
        let mut kc = [0u32; MAXL];
        for (i, it) in self.its.iter().enumerate() {
            let left = self.counts[i] - code / self.step[i] % (self.counts[i] + 1);
            if left == 0 {
                continue;
            }
            let li = if it.can { it.nk } else { it.nf };
            h += left * li;
            remain += left as usize;
            for c in 1..=self.n {
                call[c] += left * it.full[c] as u32
            }
            if it.can {
                kc[it.c] += left
            }
            live.push((i, li));
        }
        // At disposal position q, at most q additional colours can have opened.
        let mut extra = vec![u32::MAX; remain];
        let mut costs = Vec::new();
        for &(i, li) in &live {
            let it = &self.its[i];
            let kp = it.can && o >> it.c & 1 == 0;
            let fc = it.nf + self.cost(&it.full, o);
            let ck = if kp {
                it.nk + self.cost(&it.keep, o)
            } else {
                u32::MAX
            };
            let mut f: Vec<_> = (1..=self.n)
                .filter(|&c| o >> c & 1 == 0)
                .map(|c| it.full[c] as u32)
                .collect();
            f.sort_unstable_by(|a, b| b.cmp(a));
            let mut k: Vec<_> = (1..=self.n)
                .filter(|&c| o >> c & 1 == 0 && c != it.c)
                .map(|c| it.keep[c] as u32)
                .collect();
            k.sort_unstable_by(|a, b| b.cmp(a));
            let (mut sf, mut sk) = (0, 0);
            for q in 0..remain {
                if q > 0 {
                    sf += f.get(q - 1).copied().unwrap_or(0);
                    sk += k.get(q - 1).copied().unwrap_or(0)
                }
                extra[q] = extra[q].min((fc - sf).min(ck.saturating_sub(sk)).saturating_sub(li));
            }
            costs.push((fc, ck));
        }
        let lower = stored.max(h + extra.iter().sum::<u32>());
        if lower > budget {
            if self.memo.len() >= 250_000 {
                self.memo.clear();
            }
            self.memo.insert((code, o), lower);
            return lower;
        }
        let mut best = u32::MAX;
        let mut ops = Vec::new();
        for (ix, &(i, li)) in live.iter().enumerate() {
            let it = &self.its[i];
            let nm = code + self.step[i];
            let hn = h - li;
            let (fc, ck) = costs[ix];
            let kp = it.can && o >> it.c & 1 == 0;
            let mut add = |cost: u32, no: u32, priority: u32| {
                if cost + hn > budget {
                    best = best.min(cost + hn)
                } else {
                    ops.push((cost + hn, priority, nm, no, cost))
                }
            };
            if kp {
                add(ck, o | 1 << it.c, 0)
            }
            let mut present = 0u32;
            let mut kmask = 0u32;
            for c in 1..=self.n {
                if call[c] > it.full[c] as u32 {
                    present |= 1 << c
                }
                if kc[c] > (it.can && it.c == c) as u32 {
                    kmask |= 1 << c
                }
            }
            let mut rest = present & !o;
            if kp {
                rest &= !(1 << it.c)
            }
            for &cl in &self.classes {
                let av = rest & cl;
                if av != 0 {
                    rest = (rest & !cl) | (av & av.wrapping_neg())
                }
            }
            if rest & !kmask == 0 && !(kp && kmask >> it.c & 1 == 0) {
                add(fc, o, 255)
            }
            while rest != 0 {
                let c = rest.trailing_zeros() as usize;
                rest &= rest - 1;
                add(fc, o | 1 << c, 255 - (call[c] - it.full[c] as u32).min(255));
            }
        }
        ops.sort_unstable();
        for (_, _, nm, no, cost) in ops {
            let value = cost + self.go(nm, no, budget - cost);
            if value <= budget {
                return value;
            }
            best = best.min(value)
        }
        let best = best.max(lower);
        if self.memo.len() >= 250_000 {
            self.memo.clear();
        }
        self.memo.insert((code, o), best);
        best
    }
}
// J's definition from LOWER_BOUND_PROOF.md. Sparse u32 memo avoids dense
// 2^(lanes+colours) allocation and the original u8 saturation above 254.
fn joint(s: &[Lane], n: usize, threshold: Option<u32>) -> u32 {
    let mut empty = 0;
    let mut items = Vec::new();
    let mut uni = Vec::new();
    for &l in s {
        let ht = height(l);
        if ht == 0 {
            empty += 1;
            continue;
        }
        let c = ball(l, 0) as usize;
        let mut r = 1;
        while r < ht && ball(l, r) as usize == c {
            r += 1
        }
        if r == ht {
            if ht < n {
                uni.push((c, ht))
            }
            continue;
        }
        let mut keep = [0; MAXL];
        for k in r..ht {
            keep[ball(l, k) as usize] += 1
        }
        let mut full = keep;
        full[c] += r as u8;
        items.push(Item {
            c,
            nk: (ht - r) as u32,
            keep,
            nf: ht as u32,
            full,
            can: true,
        })
    }
    let check = |thr: u32| {
        let mut best = u32::MAX;
        for set in 0..1usize << uni.len() {
            let mut o = 0u32;
            let mut its = items.clone();
            let mut ok = true;
            for (i, &(c, len)) in uni.iter().enumerate() {
                if set >> i & 1 != 0 {
                    if o >> c & 1 != 0 {
                        ok = false;
                        break;
                    }
                    o |= 1 << c
                } else {
                    let mut full = [0; MAXL];
                    full[c] = len as u8;
                    its.push(Item {
                        c,
                        nk: 0,
                        keep: [0; MAXL],
                        nf: len as u32,
                        full,
                        can: false,
                    })
                }
            }
            if !ok {
                continue;
            }
            let mut dp = Joint::new(its, n);
            let present = dp.its.iter().fold(0u32, |m, it| {
                let mut m = m;
                for c in 1..=n {
                    if it.full[c] > 0 {
                        m |= 1 << c
                    }
                }
                m
            });
            let free = present & !o;
            let colors: Vec<_> = (1..=n).filter(|&c| free >> c & 1 != 0).collect();
            // Enumerate up to e free houses, collapsing symmetric colour choices.
            fn starts(
                dp: &mut Joint,
                colors: &[usize],
                i: usize,
                left: usize,
                o: u32,
                thr: u32,
                best: &mut u32,
            ) -> bool {
                let v = dp.go(0, o, thr);
                *best = (*best).min(v);
                if v <= thr {
                    return true;
                }
                if left == 0 {
                    return false;
                }
                for j in i..colors.len() {
                    let c = colors[j];
                    if dp.classes.iter().any(|&cl| {
                        cl >> c & 1 != 0 && colors[i..j].iter().any(|&d| cl >> d & 1 != 0)
                    }) {
                        continue;
                    }
                    if starts(dp, colors, j + 1, left - 1, o | 1 << c, thr, best) {
                        return true;
                    }
                }
                false
            }
            if starts(&mut dp, &colors, 0, empty, o, thr, &mut best) {
                return best;
            }
        }
        best
    };
    if let Some(thr) = threshold {
        return check(thr);
    }
    let (mut lo, mut hi) = (misplaced(s, n), 2 * (n * n) as u32);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let r = check(mid);
        if r <= mid {
            hi = r
        } else {
            lo = r.max(mid + 1)
        }
    }
    lo
}

// A feasible path in the J relaxation gives an upper bound on J, used only
// to accept a threshold (never as a pruning lower bound or evaluation value).
fn joint_greedy(s: &[Lane], n: usize) -> u32 {
    let mut empty = 0;
    let mut mono = [0usize; MAXL];
    let mut its = Vec::new();
    for &l in s {
        let ht = height(l);
        if ht == 0 {
            empty += 1;
            continue;
        }
        let c = ball(l, 0) as usize;
        let mut r = 1;
        while r < ht && ball(l, r) as usize == c {
            r += 1;
        }
        let mut keep = [0u8; MAXL];
        for k in r..ht {
            keep[ball(l, k) as usize] += 1;
        }
        let mut full = keep;
        full[c] += r as u8;
        if r == ht && ht > mono[c] {
            if mono[c] > 0 {
                let mut old = [0; MAXL];
                old[c] = mono[c] as u8;
                its.push(Item {
                    c,
                    nk: 0,
                    keep: [0; MAXL],
                    nf: mono[c] as u32,
                    full: old,
                    can: false,
                });
            }
            mono[c] = ht;
        } else {
            its.push(Item {
                c,
                nk: (ht - r) as u32,
                keep,
                nf: ht as u32,
                full,
                can: r != ht,
            });
        }
    }
    let mut o = 0u32;
    for c in 1..=n {
        if mono[c] > 0 {
            o |= 1 << c;
        }
    }
    let mut totals = [0u32; MAXL];
    for it in &its {
        for c in 1..=n {
            totals[c] += it.full[c] as u32;
        }
    }
    let mut free: Vec<_> = (1..=n).filter(|&c| o >> c & 1 == 0).collect();
    free.sort_unstable_by_key(|&c| (std::cmp::Reverse(totals[c]), c));
    for &c in free.iter().take(empty) {
        o |= 1 << c;
    }
    let mut value = 0;
    while !its.is_empty() {
        let mut best = (u32::MAX, u32::MAX, 0usize, 0u32);
        for (i, it) in its.iter().enumerate() {
            let cost = |cnt: &[u8; MAXL]| -> u32 {
                (1..=n)
                    .filter(|&c| o >> c & 1 == 0)
                    .map(|c| cnt[c] as u32)
                    .sum()
            };
            if it.can && o >> it.c & 1 == 0 {
                best = best.min((
                    it.nk + cost(&it.keep),
                    u32::MAX - totals[it.c],
                    i,
                    o | 1 << it.c,
                ));
            }
            let pick = (1..=n)
                .filter(|&c| o >> c & 1 == 0)
                .max_by_key(|&c| (totals[c] - it.full[c] as u32, std::cmp::Reverse(c)));
            let no = pick.map_or(o, |c| o | 1 << c);
            let gain = pick.map_or(0, |c| totals[c] - it.full[c] as u32);
            best = best.min((it.nf + cost(&it.full), u32::MAX - gain, i, no));
        }
        value += best.0;
        o = best.3;
        let it = its.remove(best.2);
        for c in 1..=n {
            totals[c] -= it.full[c] as u32;
        }
    }
    value
}

type Shape = Vec<(u8, u8, [u8; MAXL])>;
fn joint_cached(s: &[Lane], n: usize, threshold: Option<u32>) -> u32 {
    let mut key = Vec::with_capacity(n + 1);
    for &l in s {
        let ht = height(l);
        if ht == 0 {
            key.push((0, 0, [0; MAXL]));
            continue;
        }
        let c = ball(l, 0);
        let mut r = 1;
        while r < ht && ball(l, r) == c {
            r += 1;
        }
        let mut counts = [0; MAXL];
        for k in r..ht {
            counts[ball(l, k) as usize] += 1;
        }
        key.push((c, r as u8, counts));
    }
    key.sort_unstable();
    thread_local! { static CACHE: std::cell::RefCell<HashMap<Shape,(u32,u32)>> = std::cell::RefCell::new(HashMap::new()); }
    CACHE.with(|cell| {
        let (mut lo, mut hi) = cell.borrow().get(&key).copied().unwrap_or((0, u32::MAX));
        if lo == hi {
            return lo;
        }
        if let Some(t) = threshold {
            if hi <= t {
                return hi;
            }
            if lo > t {
                return lo;
            }
        }
        if let Some(t) = threshold {
            let upper = joint_greedy(s, n);
            hi = hi.min(upper);
            if upper <= t {
                let mut cache = cell.borrow_mut();
                if cache.len() >= 8192 {
                    cache.clear();
                }
                cache.insert(key, (lo, hi));
                return upper;
            }
        }
        let value = joint(s, n, threshold);
        match threshold {
            Some(t) if value <= t => hi = hi.min(value),
            Some(_) => lo = lo.max(value),
            None => {
                lo = value;
                hi = value;
            }
        }
        let mut cache = cell.borrow_mut();
        if cache.len() >= 8192 {
            cache.clear();
        }
        cache.insert(key, (lo, hi));
        value
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lb {
    M,
    Hd,
    J,
}
struct Config {
    n: usize,
    width: usize,
    a: f64,
    b: f64,
    d: f64,
    seed: u64,
    threads: usize,
    upper: usize,
    lb: Lb,
    eval_lb: bool,
    max_g: usize,
}
#[derive(Clone, Copy)]
struct Node {
    parent: u32,
    a: u8,
    b: u8,
    r: u8,
}
#[derive(Clone, Copy)]
struct Cand {
    hash: u64,
    parent: u32,
    h: f32,
    a: u8,
    b: u8,
    r: u8,
}
fn order(x: &Cand, y: &Cand) -> std::cmp::Ordering {
    x.h.total_cmp(&y.h)
        .then_with(|| (x.hash, x.parent, x.a, x.b, x.r).cmp(&(y.hash, y.parent, y.a, y.b, y.r)))
}
struct Layer {
    g: usize,
    base: u32,
    boards: Vec<Lane>,
}
#[derive(Clone, Copy, Default)]
struct LaneInfo {
    hash: u64,
    base: usize,
    color: usize,
    changes: u32,
    present: u32,
    prefix: usize,
}
fn lane_info(l: Lane) -> LaneInfo {
    let ht = height(l);
    let color = if ht > 0 { ball(l, 0) as usize } else { 0 };
    let mut out = LaneInfo {
        hash: 0,
        base: 0,
        color,
        changes: 0,
        present: 0,
        prefix: 0,
    };
    let mut x = 0x9e3779b97f4a7c15u64.wrapping_add(ht as u64);
    for k in 0..ht {
        let c = ball(l, k);
        x = mix(x.wrapping_mul(31).wrapping_add(c as u64 + 1));
        out.present |= 1 << c;
        if k > 0 && c != ball(l, k - 1) {
            out.changes += 1;
        }
        if out.base == k && c as usize == color {
            out.base += 1;
        }
        if out.prefix == k && c as usize == k + 1 {
            out.prefix += 1;
        }
    }
    out.hash = mix(x);
    out
}
struct BoardInfo {
    lanes: [LaneInfo; MAXL],
    best: [[(usize, usize); 3]; MAXL],
    m: i32,
    t: u32,
    spread: i32,
    prefix: usize,
    hash: u64,
    n: usize,
    finished: usize,
}
impl BoardInfo {
    fn new(s: &[Lane], n: usize) -> Self {
        let mut out = Self {
            lanes: [LaneInfo::default(); MAXL],
            best: [[(0, usize::MAX); 3]; MAXL],
            m: (n * n) as i32,
            t: 0,
            spread: -(n as i32),
            prefix: 0,
            hash: 0,
            n,
            finished: 0,
        };
        for (i, &l) in s.iter().enumerate() {
            let x = lane_info(l);
            out.lanes[i] = x;
            out.finished += (x.base == n) as usize;
            out.t += x.changes;
            out.spread += x.present.count_ones() as i32;
            out.prefix += x.prefix;
            out.hash = out.hash.wrapping_add(x.hash);
            let row = &mut out.best[x.color];
            let mut entry = (x.base, i);
            for slot in row {
                if entry.0 > slot.0 {
                    std::mem::swap(&mut entry, slot);
                }
            }
        }
        out.m -= out.best[1..=n].iter().map(|x| x[0].0 as i32).sum::<i32>();
        out
    }
    // Let A be balls outside completed lanes. M <= A and D <= A.
    // A feasible J path keeps completed lanes and fully disposes every other
    // lane (at most two units per ball), so J <= 2A as well. This certifies a
    // threshold PASS only; it is never a pruning lower bound or an eval score.
    #[cfg(test)]
    fn relaxation_upper(&self) -> u32 {
        2 * (self.n * self.n - self.finished * self.n) as u32
    }
    fn relaxation_upper_after(&self, a: usize, b: usize, x: LaneInfo, y: LaneInfo) -> u32 {
        let done = self.finished
            - (self.lanes[a].base == self.n) as usize
            - (self.lanes[b].base == self.n) as usize
            + (x.base == self.n) as usize
            + (y.base == self.n) as usize;
        2 * (self.n * self.n - done * self.n) as u32
    }
    fn changed(&self, a: usize, b: usize, x: LaneInfo, y: LaneInfo) -> (u32, u32, u32, u32, u64) {
        let olda = self.lanes[a];
        let oldb = self.lanes[b];
        let mut colors = (1u32 << olda.color) | (1 << oldb.color) | (1 << x.color) | (1 << y.color);
        colors &= !1;
        let mut m = self.m;
        while colors != 0 {
            let c = colors.trailing_zeros() as usize;
            colors &= colors - 1;
            let other = self.best[c]
                .iter()
                .find(|&&(_, i)| i != a && i != b)
                .map_or(0, |x| x.0);
            let max = other
                .max(if x.color == c { x.base } else { 0 })
                .max(if y.color == c { y.base } else { 0 });
            m += self.best[c][0].0 as i32 - max as i32;
        }
        let t = self.t - olda.changes - oldb.changes + x.changes + y.changes;
        let spread =
            self.spread - olda.present.count_ones() as i32 - oldb.present.count_ones() as i32
                + x.present.count_ones() as i32
                + y.present.count_ones() as i32;
        let total = self.prefix - olda.prefix - oldb.prefix + x.prefix + y.prefix;
        let (mut m1, mut m2) = (0, 0);
        for i in 0..=self.n {
            let p = if i == a {
                x.prefix
            } else if i == b {
                y.prefix
            } else {
                self.lanes[i].prefix
            };
            if p > m1 {
                m2 = m1;
                m1 = p
            } else if p > m2 {
                m2 = p;
            }
        }
        let hash = self
            .hash
            .wrapping_sub(olda.hash)
            .wrapping_sub(oldb.hash)
            .wrapping_add(x.hash)
            .wrapping_add(y.hash);
        (m as u32, t, spread as u32, (total - m1 - m2) as u32, hash)
    }
}

fn expand(s: &[Lane], id: u32, g: usize, seen: &Seen, c: &Config) -> Vec<Cand> {
    let mut out = Vec::new();
    let n = c.n;
    let first = s.iter().position(|&l| l == 0);
    let info = BoardInfo::new(s, n);
    for a in 0..=n {
        let ht = height(s[a]);
        if ht == 0 {
            continue;
        }
        let mut run = 1;
        while run < ht && ball(s[a], ht - 1 - run) == ball(s[a], ht - 1) {
            run += 1
        }
        if ht == n && run == n {
            continue;
        }
        for b in 0..=n {
            let room = n - height(s[b]);
            if a == b || room == 0 || (s[b] == 0 && Some(b) != first) {
                continue;
            }
            let opts = [1, run.min(room), ht.min(room)];
            for oi in 0..3 {
                let r = opts[oi];
                if opts[..oi].contains(&r)
                    || (s[b] == 0 && r == ht)
                    || g + r > c.max_g
                    || g + r >= c.upper
                {
                    continue;
                }
                let mut t = [0; MAXL];
                t[..=n].copy_from_slice(s);
                apply(&mut t, a, b, r);
                let xa = lane_info(t[a]);
                let yb = lane_info(t[b]);
                let (m, changes, spread, p, key) = info.changed(a, b, xa, yb);
                if seen.contains(&key) {
                    continue;
                }
                let t = &t[..=n];
                let budget = c.upper.saturating_sub(g + r + 1) as u32;
                if c.upper != usize::MAX && m > budget {
                    continue;
                }
                let value = match c.lb {
                    Lb::M => m,
                    Lb::Hd => {
                        if c.eval_lb {
                            hd(t, n)
                        } else if c.upper != usize::MAX {
                            if info.relaxation_upper_after(a, b, xa, yb) <= budget {
                                m
                            } else {
                                hd(t, n)
                            }
                        } else {
                            m
                        }
                    }
                    Lb::J => {
                        if c.eval_lb {
                            joint_cached(t, n, None)
                        } else if c.upper != usize::MAX {
                            if info.relaxation_upper_after(a, b, xa, yb) <= budget {
                                m
                            } else {
                                joint_cached(t, n, Some(budget))
                            }
                        } else {
                            m
                        }
                    }
                };
                if c.upper != usize::MAX && value > budget {
                    continue;
                }
                let f = if m == 0 {
                    0.0
                } else {
                    (if c.eval_lb { value } else { m }) as f64
                        + c.a * changes as f64
                        + c.b * spread as f64
                        + c.d * p as f64
                };
                let noise = mix(c.seed
                    ^ ((id as u64) << 24)
                    ^ ((a as u64) << 16)
                    ^ ((b as u64) << 8)
                    ^ r as u64);
                let jitter = ((noise >> 40) as f64 / ((1u64 << 24) as f64) * 0.02) as f32;
                out.push(Cand {
                    hash: key,
                    parent: id,
                    h: f as f32 + jitter,
                    a: a as u8,
                    b: b as u8,
                    r: r as u8,
                });
            }
        }
    }
    out
}
fn search(c: &Config, start: State) -> Option<(usize, String)> {
    if misplaced(&start[..=c.n], c.n) == 0 {
        return if c.upper > 0 {
            Some((0, String::new()))
        } else {
            None
        };
    }
    let clock = Instant::now();
    let mut seen = Seen::default();
    seen.insert(hash(&start[..=c.n]));
    let mut hist = vec![Node {
        parent: u32::MAX,
        a: 0,
        b: 0,
        r: 0,
    }];
    let mut layers: Vec<Option<Layer>> = (0..=c.n).map(|_| None).collect();
    layers[0] = Some(Layer {
        g: 0,
        base: 0,
        boards: start[..=c.n].to_vec(),
    });
    let mut buckets: Vec<Vec<Cand>> = (0..c.max_g + c.n + 1).map(|_| Vec::new()).collect();
    for cand in expand(&start[..=c.n], 0, 0, &seen, c) {
        buckets[cand.r as usize].push(cand)
    }
    let mut generated = 0usize;
    let mut peak_boards = 1;
    let mut peak_cands = 0;
    for g in 1..=c.max_g.min(c.upper.saturating_sub(1)) {
        let mut bk = std::mem::take(&mut buckets[g]);
        if bk.is_empty() {
            layers[g % (c.n + 1)] = None;
            continue;
        }
        bk.par_sort_unstable_by(order);
        let base = u32::try_from(hist.len()).expect("history exceeds u32 IDs");
        let capacity = bk.len().min(c.width);
        let mut boards = Vec::with_capacity(capacity * (c.n + 1));
        let mut fresh = Vec::with_capacity(capacity);
        for cand in bk {
            if fresh.len() >= c.width {
                break;
            }
            if !seen.insert(cand.hash) {
                continue;
            }
            let mut old = None;
            for l in layers.iter().flatten() {
                let offset = cand.parent.wrapping_sub(l.base) as usize;
                if offset < l.boards.len() / (c.n + 1) {
                    old = Some(&l.boards[offset * (c.n + 1)..(offset + 1) * (c.n + 1)]);
                    break;
                }
            }
            let mut s = [0; MAXL];
            s[..=c.n].copy_from_slice(old.expect("live parent board"));
            apply(&mut s, cand.a as usize, cand.b as usize, cand.r as usize);
            let id = u32::try_from(hist.len()).expect("history exceeds u32 IDs");
            hist.push(Node {
                parent: cand.parent,
                a: cand.a,
                b: cand.b,
                r: cand.r,
            });
            if misplaced(&s[..=c.n], c.n) == 0 {
                let mut mac = Vec::new();
                let mut id = id;
                while id != 0 {
                    let x = hist[id as usize];
                    mac.push(x);
                    id = x.parent
                }
                mac.reverse();
                let mut path = String::new();
                for x in &mac {
                    for _ in 0..x.r {
                        path.push(char::from_digit(x.a as u32, 36).unwrap());
                        path.push(char::from_digit(x.b as u32, 36).unwrap())
                    }
                }
                assert_eq!(path.len() / 2, g);
                eprintln!("N={} W={} seed={} threads={} moves={} pours={} elapsed={:.3} kept={} seen={} peak_boards={} peak_candidates={}",c.n,c.width,c.seed,c.threads,g,mac.len(),clock.elapsed().as_secs_f64(),hist.len(),seen.len(),peak_boards,peak_cands);
                return Some((g, path));
            }
            boards.extend_from_slice(&s[..=c.n]);
            fresh.push(id);
        }
        layers[g % (c.n + 1)] = Some(Layer { g, base, boards });
        let layer = layers[g % (c.n + 1)].as_ref().unwrap();
        let all: Vec<_> = fresh
            .par_iter()
            .enumerate()
            .map(|(i, &id)| {
                expand(
                    &layer.boards[i * (c.n + 1)..(i + 1) * (c.n + 1)],
                    id,
                    g,
                    &seen,
                    c,
                )
            })
            .collect();
        for children in all {
            generated += children.len();
            for child in children {
                buckets[g + child.r as usize].push(child)
            }
        }
        // In a bucket d costs ahead, at most (d-1)*W hashes can become
        // seen before it is selected. Keeping its best d*W distinct unseen
        // hashes therefore preserves the eventual top W, even if new
        // candidates arrive. This is safe unlike prematurely capping at W.
        for (offset, bucket) in buckets[g + 1..=(g + c.n).min(c.max_g)]
            .iter_mut()
            .enumerate()
        {
            let cap = c.width * (offset + 1);
            if bucket.len() > c.width * 4 || bucket.len() > cap {
                bucket.retain(|x| !seen.contains(&x.hash));
                bucket.par_sort_unstable_by(|x, y| x.hash.cmp(&y.hash).then_with(|| order(x, y)));
                bucket.dedup_by(|x, y| x.hash == y.hash);
                if bucket.len() > cap {
                    bucket.select_nth_unstable_by(cap, order);
                    bucket.truncate(cap);
                    if bucket.capacity() > 2 * cap {
                        bucket.shrink_to(cap);
                    }
                }
            }
        }
        for slot in &mut layers {
            if slot.as_ref().is_some_and(|l| l.g + c.n <= g) {
                *slot = None
            }
        }
        peak_boards = peak_boards.max(
            layers
                .iter()
                .flatten()
                .map(|l| l.boards.len() / (c.n + 1))
                .sum(),
        );
        peak_cands = peak_cands.max(
            buckets[g + 1..=(g + c.n).min(c.max_g)]
                .iter()
                .map(Vec::len)
                .sum(),
        );
        if g % 50 == 0 {
            eprintln!("g={g} kept={} generated={generated} boards_peak={peak_boards} candidates_peak={peak_cands} elapsed={:.1}",hist.len(),clock.elapsed().as_secs_f64())
        }
    }
    eprintln!(
        "no solution N={} W={} elapsed={:.3} kept={} seen={} peak_boards={} peak_candidates={}",
        c.n,
        c.width,
        clock.elapsed().as_secs_f64(),
        hist.len(),
        seen.len(),
        peak_boards,
        peak_cands
    );
    None
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len()>=3,"pour N W [--a A --b B --d D --seed S --threads T --start LANES --upper U --lb m|hd|j --eval-lb]");
    let n = args[1].parse::<usize>().unwrap();
    let width = args[2].parse().unwrap();
    assert!((1..=24).contains(&n) && width > 0);
    let mut c = Config {
        n,
        width,
        a: 0.25,
        b: 0.5,
        d: 1.0,
        seed: 1,
        threads: 1,
        upper: usize::MAX,
        lb: Lb::M,
        eval_lb: false,
        max_g: 3 * n * n,
    };
    let mut st = None;
    let mut i = 3;
    while i < args.len() {
        if args[i] == "--eval-lb" {
            c.eval_lb = true;
            i += 1;
            continue;
        }
        assert!(i + 1 < args.len(), "missing argument");
        let v = &args[i + 1];
        match args[i].as_str() {
            "--a" => c.a = v.parse().unwrap(),
            "--b" => c.b = v.parse().unwrap(),
            "--d" => c.d = v.parse().unwrap(),
            "--seed" => c.seed = v.parse().unwrap(),
            "--threads" => c.threads = v.parse().unwrap(),
            "--start" => st = Some(v.clone()),
            "--upper" => c.upper = v.parse().unwrap(),
            "--lb" => {
                c.lb = match v.as_str() {
                    "m" => Lb::M,
                    "hd" => Lb::Hd,
                    "j" => Lb::J,
                    _ => panic!("unknown lower bound"),
                }
            }
            "--max-g" => c.max_g = v.parse().unwrap(),
            _ => panic!("unknown option {}", args[i]),
        }
        i += 2
    }
    assert!(c.threads > 0 && c.a.is_finite() && c.b.is_finite() && c.d.is_finite());
    let s = st.map_or_else(|| initial(n), |st| parse(n, &st));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(c.threads)
        .build()
        .unwrap();
    if let Some((moves, path)) = pool.install(|| search(&c, s)) {
        println!("N={n} moves={moves}");
        println!("{path}")
    } else {
        std::process::exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    fn canon(mut s: State, n: usize) -> State {
        s[..=n].sort_unstable();
        s
    }
    #[test]
    fn bounds_against_all_three_colour_exact_distances() {
        let n = 3;
        let mut goal = [0; MAXL];
        for c in 1..=n {
            for _ in 0..n {
                goal[c] = push(goal[c], c as u8)
            }
        }
        let goal = canon(goal, n);
        let mut distance = HashMap::new();
        distance.insert(goal, 0u32);
        let mut todo = VecDeque::from([goal]);
        let mut checked = 0;
        while let Some(s) = todo.pop_front() {
            let d = distance[&s];
            let m = misplaced(&s[..=n], n);
            let h = hd(&s[..=n], n);
            let j = joint(&s[..=n], n, None);
            let cap = BoardInfo::new(&s[..=n], n).relaxation_upper();
            assert!(j <= cap && h <= cap);
            assert!(joint_greedy(&s[..=n], n) >= j);
            assert!(m <= h && h <= d && j <= d, "{m} {h} {j} {d}");
            for t in j.saturating_sub(2)..=j + 2 {
                let r = joint(&s[..=n], n, Some(t));
                assert_eq!(joint_cached(&s[..=n], n, Some(t)) <= t, j <= t);
                assert_eq!(r <= t, j <= t);
                if r > t {
                    assert!(r <= j)
                }
            }
            assert_eq!(joint_cached(&s[..=n], n, None), j);
            checked += 1;
            for a in 0..=n {
                if height(s[a]) == 0 {
                    continue;
                }
                for b in 0..=n {
                    if a == b || height(s[b]) == n {
                        continue;
                    }
                    let mut t = s;
                    apply(&mut t, a, b, 1);
                    let t = canon(t, n);
                    if let std::collections::hash_map::Entry::Vacant(e) = distance.entry(t) {
                        e.insert(d + 1);
                        todo.push_back(t)
                    }
                }
            }
        }
        assert!(checked > 1000);
        eprintln!("exact-distance states checked={checked}");
    }
    #[test]
    fn incremental_matches_full_scan() {
        for n in 2..=16 {
            let mut state = initial(n);
            let mut rng = n as u64;
            for _ in 0..200 {
                let info = BoardInfo::new(&state[..=n], n);
                rng = mix(rng + 1);
                let a = (rng as usize) % (n + 1);
                rng = mix(rng + 1);
                let b = (rng as usize) % (n + 1);
                if a == b || height(state[a]) == 0 || height(state[b]) == n {
                    continue;
                }
                let room = n - height(state[b]);
                rng = mix(rng + 1);
                let r = 1 + (rng as usize) % height(state[a]).min(room);
                let mut next = state;
                apply(&mut next, a, b, r);
                let (m, t, spread, p, h) =
                    info.changed(a, b, lane_info(next[a]), lane_info(next[b]));
                assert_eq!(m, misplaced(&next[..=n], n));
                assert_eq!((t, spread, p), features(&next[..=n], n));
                assert_eq!(h, hash(&next[..=n]));
                let active: usize = next[..=n]
                    .iter()
                    .filter(|&&l| height(l) != n || (1..n).any(|k| ball(l, k) != ball(l, 0)))
                    .map(|&l| height(l))
                    .sum();
                assert_eq!(
                    info.relaxation_upper_after(a, b, lane_info(next[a]), lane_info(next[b])),
                    2 * active as u32
                );

                state = next;
            }
        }
    }
    #[test]
    fn recorded_lower_bound_samples() {
        for (n, file, stride) in [(4, "dump4.txt", 127), (5, "band5.txt", 31)] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../learn")
                .join(file);
            let data = std::fs::read_to_string(path).unwrap();
            let mut count = 0;
            for line in data.lines().step_by(stride).take(1100) {
                let cols: Vec<_> = line.split_whitespace().collect();
                let text = cols[4..].join(",");
                let s = parse(n, &text);
                let m = cols[2].parse::<u32>().unwrap();
                let d = cols[3].parse::<u32>().unwrap();
                assert_eq!(misplaced(&s[..=n], n), m);
                assert_eq!(hd(&s[..=n], n), m + d);
                let j = joint(&s[..=n], n, None);
                let cap = BoardInfo::new(&s[..=n], n).relaxation_upper();
                assert!(j <= cap && m + d <= cap);
                assert!(j <= cols[1].parse::<u32>().unwrap());
                assert!(joint_greedy(&s[..=n], n) >= j);
                for t in j.saturating_sub(3)..=j + 3 {
                    let v = joint_cached(&s[..=n], n, Some(t));
                    assert_eq!(v <= t, j <= t);
                    if v > t {
                        assert!(v <= j);
                    }
                }
                assert_eq!(joint_cached(&s[..=n], n, None), j);
                count += 1;
            }
            eprintln!("N={n} recorded bound samples={count}");
        }
    }
    #[test]
    fn initial_bounds_and_hash() {
        for n in 2..=16 {
            let s = initial(n);
            let expected = if n == 2 { 3 } else { (3 * n * n - n - 4) / 2 } as u32;
            assert_eq!(joint(&s[..=n], n, None), expected, "n={n}");
            let mut p = s;
            p[..=n].reverse();
            assert_eq!(hash(&s[..=n]), hash(&p[..=n]));
        }
        assert_eq!(std::mem::size_of::<Node>(), 8);
        assert_eq!(std::mem::size_of::<Cand>(), 24);
    }
    #[test]
    fn assignment_matches_subset_dp() {
        let mut x = 1u64;
        for n in 1..=8 {
            for _ in 0..100 {
                let mut rows = vec![[0u8; MAXL]; n];
                for row in &mut rows {
                    for c in row.iter_mut().take(n + 1) {
                        x = mix(x + 1);
                        *c = (x % 20) as u8
                    }
                }
                let mut dp = vec![u32::MAX; 1 << n];
                dp[0] = 0;
                for mask in 1usize..1 << n {
                    let col = mask.count_ones() as usize - 1;
                    for (i, row) in rows.iter().enumerate() {
                        if mask >> i & 1 != 0 {
                            dp[mask] = dp[mask].min(dp[mask ^ (1 << i)] + row[col] as u32)
                        }
                    }
                }
                assert_eq!(assignment(&rows, 0, n), dp[(1 << n) - 1]);
            }
        }
    }
}
