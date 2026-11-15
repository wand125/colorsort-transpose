# Ball-sort transposition: shortest solutions for N colours in N+1 tubes

日本語: [README.ja.md](README.ja.md)

Website: <https://wand125.github.io/colorsort-transpose/> — overview, paper ([English](https://wand125.github.io/colorsort-transpose/paper.html), [日本語](https://wand125.github.io/colorsort-transpose/paper.ja.html)), [data](https://wand125.github.io/colorsort-transpose/data.html) and an [animated replay](https://wand125.github.io/colorsort-transpose/play.html) of the solutions.

Computer-assisted results on a ball-sort ("colour sort") puzzle from one fixed starting position:
exact optimal move counts for up to 6 colours, a general lower bound, verified upper bounds for up to 24 colours,
and the code that produced them.

## The problem

- N colours, N balls of each colour, N+1 tubes ("lanes") of capacity N.
- Start: lane 0 is empty; each of lanes 1..N holds the colours 1, 2, …, N from bottom to top.
- A move takes the top ball of one lane and puts it on any other lane that is not full. There is no colour restriction.
- Goal: every lane is empty or holds N balls of one colour. T(N) is the minimum number of moves.

Every row of the starting position is monochrome and the goal makes every column monochrome,
so this is a "transposition" of N stacks using one spare stack.

## Results

<!-- bounds:start -->
| N | lower bound | upper bound | status | (2N−1)(N−1) |
|---|---|---|---|---|
| 2 | 3 | 3 | **exact** | 3 |
| 3 | 10 | 10 | **exact** | 10 |
| 4 | 21 | 21 | **exact** | 21 |
| 5 | 36 | 36 | **exact** | 36 |
| 6 | 55 | 55 | **exact** | 55 |
| 7 | 74 | 78 | open | 78 |
| 8 | 93 | 106 | open | 105 |
| 9 | 117 | 141 | open | 136 |
| 10 | 144 | 180 | open | 171 |
| 11 | 174 | 229 | open | 210 |
| 12 | 208 | 272 | open | 253 |
| 13 | 245 | 380 | open | 300 |
| 14 | 285 | 463 | open | 351 |
| 15 | 328 | 547 | open | 406 |
| 16 | 374 | 646 | open | 465 |
| 17 | 423 | 743 | open | 528 |
| 18 | 475 | 863 | open | 595 |
| 19 | 530 | 1004 | open | 666 |
| 20 | 588 | 1157 | open | 741 |
| 21 | 649 | 1336 | open | 820 |
| 22 | 713 | 1477 | open | 903 |
| 23 | 780 | 1632 | open | 990 |
| 24 | 850 | 1873 | open | 1081 |
<!-- bounds:end -->

The source of this table is [bounds.tsv](bounds.tsv). The last column is the quadratic that fits the exact values; it is a guide, not a claim.

- **Exact values.** T(2..6) = 3, 10, 21, 36, 55. The values for N ≤ 5 were known from exhaustive search (see Prior work).
  T(6) = 55 is established here: a 55-move solution exists, and a pruned breadth-first search with an admissible
  lower bound shows that no solution of 54 moves or fewer exists (4.7 million states).
- **Seven to ten colours.** 74 ≤ T(7) ≤ 78, 93 ≤ T(8) ≤ 106, 117 ≤ T(9) ≤ 141 and 144 ≤ T(10) ≤ 180; the lower bounds come from the same pruned search, and the upper bounds are explicit solutions.
- **General lower bound.** T(N) ≥ (3N² − N − 4)/2 for all N.
- **Upper bounds.** Every upper bound in the table comes with an explicit move sequence in [solutions/](solutions/).
  For N = 9, 10, 11 they come from beam search followed by re-optimisation of solution tails. The 8-colour (106) and 12-colour (272)
  solutions are earlier recorded solutions that this project has not improved; the 12-colour one was found by a search over macro-moves,
  and the origin of the 8-colour one is not recorded. Both replay correctly. For N = 13…24 the bounds come from a construction (complete the upper half of the colours
  greedily, pouring two half-emptied lanes together to free a lane), then either a beam search on the remaining half or a replay of a known
  solution for half as many colours on its top layer followed by beam search. The values for N ≥ 13 are rough.
- **Growth.** T(N) = Θ(N²) follows from Szegedy and Yu (see Prior work). The constant is open:
  the lower bound gives 1.5, and the fitted quadratic would give 2.

## Lower bounds

[LOWER_BOUND_PROOF.md](LOWER_BOUND_PROOF.md) (in Japanese) defines three bounds and proves that each never exceeds the true distance from any state.

- **h + D.** h counts balls that must move at least once. D bounds the number of balls that must move at least twice:
  a ball moves exactly once only if it goes straight to the final lane of its colour, and a lane cannot serve as a final lane
  until the balls that have to leave it have left.
- **h + D′.** Same argument, but the final lane of each colour is fixed once for the whole solution.
- **J.** Minimises (balls that move) + (balls that move at least twice) jointly over one consistent choice of final lanes,
  and stops treating finished lanes as free.

Checked against exact distances: no violation on all 37.45 million states for N = 4. The mean gap between J and the true distance there is 0.92.
With J, the search that proves T(6) ≥ 55 visits 4.7 million states; with h + D the search for T(6) ≥ 52 alone visited 325 million.
The logs of the deciding runs are `results/logs/prove6J_55_final.log` (T(6) ≥ 55) `results/logs/prove7J_74.log` (T(7) ≥ 74), `results/logs/prove8J_93.log` (T(8) ≥ 93) , `results/logs/prove9J_117.log` (T(9) ≥ 117) and `results/logs/prove10J_144.log` (T(10) ≥ 144).

## Verify

```
python3 tools/verify_all.py
```

replays every stored solution from the starting position, checks that it reaches the goal with the stated number of moves,
and checks the upper bounds in `bounds.tsv` against them. Solutions are strings in base 36, two characters (from, to) per move.

The lower-bound searches are in `lowerbound/` (Rust):

```
cd lowerbound && cargo build --release
./target/release/colorlb check 4                      # compare the bound with exact distances on all states, N = 4
COLORLB_JOINT=1 ./target/release/colorlb prove 6 55   # no solution with at most 54 moves, so T(6) ≥ 55
```

## Layout

| Path | Contents |
|---|---|
| `bounds.tsv` | Lower and upper bounds for N = 2…24 with their sources |
| `solutions/` | Best known move sequences |
| `LOWER_BOUND_PROOF.md` | Proofs for h + D, D′ and J |
| `lowerbound/` | Lower bounds, pruned breadth-first proof search, A* from arbitrary states |
| `beam/` | Beam search for upper bounds (Rust, rayon); `big` handles up to 24 colours |
| `tools/` | Verification, the construction for large N, re-optimisation of solution tails, Python prototypes of D′ and J |
| `learn/` | Exact distances for N = 4 and a band of states for N = 5, used to test bounds and evaluation functions |
| `results/` | Search results and proof logs |
| `docs/` | The website (GitHub Pages). `python3 tools/build_site.py` regenerates its tables and data from `bounds.tsv` and `solutions/` |
| `archive/` | Earlier versions of the beam search |

## Prior work

- An anonymous page, <https://piles-five.vercel.app/>, treats the same problem as "matrix transposition", gives 3, 10, 21, 36 for N ≤ 5
  by exhaustive search, and conjectures (2N−1)(N−1) from a quadratic fit.
- M. Szegedy and J. Yu, "Rubik Tables and Object Rearrangement", arXiv:2002.04979: n stacks of depth d with one empty stack can be
  rearranged in O(nd) moves (for d ≤ n^(m/2), m constant), which gives T(N) = Θ(N²). No explicit small constant, and the lower bound there is Ω(nd).
- T. Ito et al., "Sorting Balls and Water: Equivalence and Computational Complexity", FUN 2022 (arXiv:2202.09495), and related work
  study versions where a ball may only be placed on a ball of the same colour.

We did not find the general lower bound, T(6), or bounds for N ≥ 7 in the literature. Corrections are welcome.

## License

MIT. See [LICENSE](LICENSE).
