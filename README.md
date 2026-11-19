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
| 7 | 75 | 78 | open | 78 |
| 8 | 95 | 106 | open | 105 |
| 9 | 117 | 140 | open | 136 |
| 10 | 145 | 179 | open | 171 |
| 11 | 175 | 224 | open | 210 |
| 12 | 208 | 272 | open | 253 |
| 13 | 245 | 329 | open | 300 |
| 14 | 285 | 387 | open | 351 |
| 15 | 328 | 456 | open | 406 |
| 16 | 374 | 525 | open | 465 |
| 17 | 423 | 611 | open | 528 |
| 18 | 475 | 715 | open | 595 |
| 19 | 530 | 790 | open | 666 |
| 20 | 588 | 911 | open | 741 |
| 21 | 649 | 1019 | open | 820 |
| 22 | 713 | 1113 | open | 903 |
| 23 | 780 | 1305 | open | 990 |
| 24 | 850 | 1451 | open | 1081 |
<!-- bounds:end -->

The source of this table is [bounds.tsv](bounds.tsv). The last column is the quadratic that fits the exact values; it is a guide, not a claim.

- **Exact values.** T(2..6) = 3, 10, 21, 36, 55. The values for N ≤ 5 were known from exhaustive search (see Prior work).
  T(6) = 55 is established here: a 55-move solution exists, and a pruned breadth-first search with an admissible
  lower bound shows that no solution of 54 moves or fewer exists (4.7 million states).
- **Seven to ten colours.** 75 ≤ T(7) ≤ 78, 95 ≤ T(8) ≤ 106, 117 ≤ T(9) ≤ 140 and 145 ≤ T(10) ≤ 179; the lower bounds come from the same pruned search, and the upper bounds are explicit solutions.
- **General lower bound.** T(N) ≥ (3N² − N − 4)/2 for all N.
- **Upper bounds.** Every upper bound in the table comes with an explicit move sequence in [solutions/](solutions/).
  For N = 9, 10, 11 they come from beam search followed by re-optimisation of solution tails. The 8-colour (106) and 12-colour (272)
  solutions are earlier recorded solutions that this project has not improved; the 12-colour one was found by a search over macro-moves,
  and the origin of the 8-colour one is not recorded. Both replay correctly. The 13- and 14-colour bounds (329, 387) come from the same pour-based beam search that found the 12-colour solution (widths 10^4 and 3·10^4; different random seeds for tie-breaking). For N = 15…24 the bounds come from a construction (complete the upper half of the colours
  greedily, pouring two half-emptied lanes together to free a lane), then either a beam search on the remaining half or a replay of a known
  solution for half as many colours on its top layer followed by beam search. For N = 16 re-solving the tail with a beam of 10^6 states per layer shortened the constructed 646-move solution to 643, and joining this with SAT window re-solves (646 → 635) at the common position that minimises the total gives 632 (and long-window beam re-solves 613; the pour-based beam with width 10^4 gives 613 directly and 611 after window re-solves, and with width 3·10^4 it gives 562 (560 after 48-move window re-solves); the parallel reimplementation with width 10^5 gives 541 (538 after 48-move window re-solves), and 525 with re-tuned evaluation weights; for N = 17 the pour-based beam with width 2000 gives 715 and with width 3·10^4 gives 677 (the parallel reimplementation gives 657 with width 10^5 and 650 with width 2·10^5, and 625 with width 10^5 and re-tuned evaluation weights), shorter than the construction's 743, for N = 18 the parallel reimplementation with width 10^5 gives 757, and 719 with re-tuned evaluation weights, against the construction's 863, for N = 19 it gives 998 (the parallel reimplementation gives 900 with width 10^5, 883 with width 2·10^5, and 853 with width 10^5 and re-tuned evaluation weights) against 1004, for N = 20 the parallel reimplementation with width 10^5 gives 1027 (992 with re-tuned evaluation weights) against the construction's 1157, and for N = 21 1302 (1129 with the parallel reimplementation) against 1336; for N = 22, 23 and 24 the parallel reimplementation gives 1355, 1568 and 1672 against the construction's 1477, 1632 and 1873), and for N = 15 checking every 12-move window with a SAT solver for a shorter equivalent sequence shortened 547 to 536; joined with a beam re-solve of the tail (547 → 543) this gives 532, and a further tail re-solve gives 531; the pour-based beam search (width 3·10^4) then gave 483 directly, re-solving its tail gave 482, and a parallel reimplementation with width 10^5 gave 468, and 457 with re-tuned evaluation weights. The values for N ≥ 13 are rough.
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
The logs of the deciding runs are `results/logs/prove6J_55_final.log` (T(6) ≥ 55) `results/logs/prove7J_75.log` (T(7) ≥ 75), `results/logs/prove8J_95.log` (T(8) ≥ 95) , `results/logs/prove9J_117.log` (T(9) ≥ 117) `results/logs/prove10J_145.log` (T(10) ≥ 145) and `results/logs/prove11J_175.log` (T(11) ≥ 175).

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
