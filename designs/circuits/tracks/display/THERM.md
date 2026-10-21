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
at oz-4 shared by both columns), the chain on top of the shorter column. It builds (1464 blocks)
and its box is ~20x13x27 = 7,000, but the seven lanes do not route: every lane needs its own
z-plane (two lanes at the same lamp row would stand on each other), and then
- a lane's north run at its lamp column crosses the z-planes of every lane in front of it: for two
  lanes whose rows differ by 1, the one with the SMALLER z must not have its west run reach the
  other's lamp column (lx_small > lx_big). Solvable: f 1, e 2, g 3, a 4, d 5, b 6, c 7.
- but the descent (dust steps down from the strip's level to its lamp row) and the west run cannot
  share a z-plane: a descent step one level above the run stands on the run's repeater. Descending
  west overshoots the lamp column for column A (dy up to 6, only 2 cells of room), descending east
  puts the run back under the descent. The fix is a second plane per lane (descend at z+1, run at
  z) or a 2x2 spiral descent (1 level per cell, in (x, z) = (cx, cx+1) x (z, z+1), which does not
  self-block), with the cross-lane constraints re-checked.

## Volume ideas beyond the tower
- Share one ruler between two segments (one per side): 7 rulers -> 4.
- The dense form: T_t as tap blocks K_t, a window one comparator with rear K_lo and a dust standing
  on K_hi+1 as its side (both switch with K: still glitch-free); needs a 3D placer/router.
