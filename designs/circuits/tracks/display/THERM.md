# clk-obs9: the thermometer decoder after the face rule (2026-10-21)

`mctas circuit display therm [--flat|--narrow|--tower] --out F`
(crates/circuit-display/src/therm.rs), then `display equalize F --at=<the chain's output dust>`
and `display settle F --frames 400`.

| design | blocks | box (volume) | latency | glitches | game |
|---|---|---|---|---|---|
| therm-v1 | 1989 | 41x8x80 (26,240) | 178 gt | 28 | robust |
| therm-v2 (equalized) | 1989 | 41x8x80 (26,240) | 178 gt | 0 | robust |
| therm-v3 (`--flat`) | 1793 | 47x7x52 (17,108) | 130 gt | 0 | robust |
| **therm-v4** (`--narrow`) | **1465** | **47x7x39 (12,831)** | 108 gt | **0** | robust |

## How it works
- The analog chain (code w [4,1,8,2], every bit complemented, `astack::build_chain` with the
  inputs brought to one back row: `therm::chain_back`) turns the four bits into one strength
  s = 15 - (4 a0' + a1' + 8 a2' + 2 a3').
- Per segment a STRIP (`build_strip`, frame u along the ruler, h up, v across): a dust ruler from
  s (cell j carries s - j) and, per run [lo, hi] of that segment's ON strengths, ONE subtract
  comparator C at (lo-1, 0, 2 sg): rear a delay-2 tap repeater on ruler cell lo-1 (T_lo), side the
  tap on cell hi (T_hi+1; a delay-1 tap plus a delay-1 repeater into C's side, 4 gt either way).
  A run touching s = 0 takes a redstone block as rear, a run touching 15 has no side.
  Both of a window's inputs read the same dust, so they switch in the same tick: no glitch from
  the window itself, and none at all once s moves in one step (`equalize`).
- Runs alternate between the two sides of the ruler (a multi-cell run's side feeder ends in a dust,
  which would feed the next comparator's side).
- The OR row of each side: a block under each comparator's output, dust on top along u, ONE
  repeater inside the row (the farthest comparator within 15 of it, the row's head within 15 of
  the repeater), then an L at the head; both sides' dust power the lane-entry block (v -3..3,
  therm-v4; therm-v1..v3 had a merge repeater and a return row, v -5..5).
- Lanes: a repeater chain per lamp at the lamp's own row, from the strip head to the face; the
  lamps are the judge's (air cells at z = 0, outside the box).
- Spines: two lossless lines (`walk`: comparator, block; `uN`/`dN` change level in 2 cells) carry
  s from the chain to every strip's hub. A level step's dust must stay 2 cells away from a band
  (it joined a strip's return row and broke four segments).

## therm-v5, the tower (`--tower`, NOT working yet: lane routing)
Strips along +z in two columns (x = 3 and 13) stacked every 3 levels, s carried down ONE lossless
column between them (`dE dW` zigzag, 2 cells per level, a branch to every hub through a z-detour
at oz-4 shared by both columns), the chain on top of the shorter column. It builds (~1460 blocks)
and its box is ~20x13x27 = 7,000 — but the seven lanes do not route. What is settled:
- every lane needs its own z-plane (two lanes at the same lamp row would stand on each other);
- a descent must not share a plane with its own west run (a dust one level above the run stands on
  the run's repeater); the 2x2 dust spiral (x cx..cx+1, z zl..zl+1, one level per cell) is the
  descent that never blocks itself, and is what `--tower` builds now;
- what is left is the crossings: a lane's north run at its lamp column passes through every plane
  in front of it, and the lanes' west runs sweep the lamp columns at their own row. A search over
  the 7 segment-to-slot assignments x the 7 lane planes (20k random draws, `tower()` counts
  clashes and unsupported cells) gets to 11 clashes, not 0 — the crossings need a real router (or
  lanes that travel at their own y and only drop to the lamp row inside the face plane, where the
  8 non-lamp face cells may hold design blocks).

## Volume ideas beyond the tower
- **Shared rulers (7 -> 4, est. 28x7x39 ≈ 7,600)**: one ruler per PAIR of segments, segment A's
  runs on the +v side, B's on the -v side (each side keeps its own OR row and lane head), so the
  x extent halves. Blocked by one case, and `therm-orders` now prints it: all runs of a segment
  on one side means a run's side feeder (dust at u = lo+1..hi) sits next to the following run's
  comparator at u = lo'-1 whenever the two runs are separated by exactly ONE off value, and a
  comparator ORs its two sides, so that neighbour's tap suppresses it forever. A multi-cell run
  ends its feeder in a BLOCK (a comparator's side ignores blocks: the tap points into the block,
  the block powers the dust one cell west with 15) — but a SINGLE-cell run's only feeder cell must
  stay dust, and **no code (of the 48 the chain can build) avoids a single-cell run followed by a
  gap-1 run** (`display therm-orders` prints the surviving codes: none). The ways out, untried:
  a comparator turned along u (rear a block the tap points into, one side the tap row, the other
  the OR row's support stone, which it ignores) for the colliding window; or the colliding run's
  feeder taken one level down (dust under the tap's block) and back up two cells west; or pairing
  only the segments whose runs leave room and sharing 5 rulers instead of 4.
- The dense form: T_t as tap blocks K_t, a window one comparator with rear K_lo and a dust standing
  on K_hi+1 as its side (both switch with K: still glitch-free); needs a 3D placer/router.
- Lane geometry for any smaller packing (worked out, not built): order the seven lanes by z so
  their lamp columns never increase (f2 e2 | a1 g2 d1 | c0 b0 by column), and a lane's west run
  then never crosses a farther lane's north run; within a column the rows already differ by 2, so
  no repeater stands on another's support. Pair the segments per band as {f,e}, {a,g}, {d,c}, {b}
  to keep every band's two rows within 2, which a 4-cell staircase in the lane can absorb.
