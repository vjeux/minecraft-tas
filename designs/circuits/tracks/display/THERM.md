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
| therm-v5 (`--tower`, clk-obs10) | 1441 | 19x13x28 (6,916) | 116 gt | 0 | robust |
| therm-v6 (`--tower --oz 10`) | 1428 | 19x13x24 (5,928) | 108 gt | 0 | robust |
| therm-v7 (`--oz 10 --cxb 14`) | 1411 | 18x13x24 (5,616) | 104 gt | 0 | robust |
| therm-v8 (`--oz 9 --cxb 14`) | 1401 | 18x13x23 (5,382) | 104 gt | 0 | robust |
| therm-v9 (`--shared`, 5 rulers) | 1176 | 18x10x23 (4,140) | 96 gt | 0 | robust |
| therm-v10 (`--shared`, 4 rulers, chain behind) | 1033 | 18x7x29 (3,654) | 88 gt | 0 | robust |
| therm-v11 (v10, chain tucked behind the shortest strip; search by volume) | 1062 | 18x7x27 (3,402) | 86 gt | 0 | robust |
| **therm-v12** (v11, another pairing: e alone, df gc ab) | **1046** | **18x7x27 (3,402)** | 84 gt | **0** | robust |

3,402 is this layout's floor in 1,600 more searched bases (`--orders 6`, seeds 11 and 23: none
below 3,402; the chain overlaps the strips by at most one layer).

Regenerate a tower design: `display therm --tower [--shared] --oz 9 --cxb 14 --groups G --slots S
--order O --tries 1 --name N --out F` (the search prints G/S/O; without them it draws `--tries`
random ones), then `display equalize F --at=<chain hub dust>` (the dust at the chain's output:
x 13 or 7, y = the s line's top, z = the input row's z - 2) and `display settle F --frames 400`.

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

## The tower (clk-obs10: therm-v5..v10, `--tower`)
Strips along +z in two columns (A at x 3, B at x `--cxb`), stacked every 3 levels; the lane zone
in front (z 1 ..= oz-5), the strips' heads at z oz-4.
- **Lanes: an A* router** (crates/circuit-display/src/route.rs). A lane is a chain of elements
  (repeater, powered turn block, dust; dust steps reserve their corner as air) from the strip's
  output repeater to a north repeater or block behind its lamp. Conservative interaction model:
  every emitter/reader pair must be (lane i, lane i+1) of one lane (dust reads all 6 neighbours
  and emits into all 4 sides + below; a block passes on whatever powers it; base cells emit and
  read everything). Lanes are routed one after the other in a random order; `--tries` draws slot
  assignments x orders. R-K-R-K (repeater, block) beats straight repeaters (no supports). The
  lane zone is 4 deep at best (`--oz 9`; 3 deep never routes).
- **s line: a searched lossless path** (`spiral_search`) in the gap between the columns: moves
  `dN/dS/dE/dW` (dust under the block, comparator one level down, block) and flat `N/S/E/W`,
  every comparator and dust on stone, clear of the strips (no base neighbour but stone; dust: no
  strongly powered stone and no diagonal dust link), no line dust beside a line comparator's side,
  and before leaving a hub level both hub-row ends (z oz-4) carry s; the branches run along the hub
  row then south into each hub. A row end at even x is a DUST beside the line's block (parity).
  Settled: a 3-wide gap (B at x 13) has NO line: right below a hub row the next descent's
  comparator either stands on the row's comparator or sits beside a strip's OR-row dust (the old
  `dE dW` zigzag was invalid anyway: each comparator stood on the next). B at x 14 works.
- **Glitches**: a strip side's OR row has one repeater; a window nearer the head than it bypasses
  it and its output changes 2 gt early, so a handover between windows on either side of the
  repeater dips. The repeater now sits just before the nearest window when the farthest is within
  15 (HEAD_REP), else the bypassing windows' input repeaters get +1 delay. `display glitches`
  windows were too short for 100+ gt designs (fixed; it prints change frames).
- **Shared rulers** (`--shared`): two segments per ruler, each side one segment with its own head
  (the branch side's head takes the centre entry v 0, the other v +-1: a branch comparator above
  an output repeater at v +-1 would stand on it); a multi-cell run's feeder ends in a block; an
  s == 0 single run takes its side from the head end (a dust on the hub feeds a delay-3 repeater
  into a dust beside C at u -2), so only a single run ABOVE 0 followed by a gap-1 run forces a
  segment alone: code [4,1,8,2] -> only e (`therm-orders` lists codes; 4 rulers is the floor).
- Chain: over column B when A is taller (v5..v9), else behind the strips with s fed north along
  the gap at x 9 (v10: costs 6 layers of z).

## Next
- v12's chain behind costs 18x7x4: put the chain in a 5-wide gap (`--cxb 15`, width +1)
  rotated, its 4 inputs brought to the back face by routed repeater lines (generalize the lane
  router's goal: a cell + direction instead of "behind a lamp"): est. 19x7x23 = 3,059.
- A single run above 0 followed by a gap-1 run (e): take that window one level down (rear and side
  repeaters reading the ruler cells' support blocks, its output into a block under a dipped OR-row
  dust) - then 4 rulers = 3 pairs + 1 single for any code.
- Shared-ruler strips are still 7 wide; the dense form below would narrow them.
- The dense form: T_t as tap blocks K_t, a window one comparator with rear K_lo and a dust standing
  on K_hi+1 as its side (both switch with K: still glitch-free); needs a 3D placer/router.
- Lane geometry for any smaller packing (worked out, not built): order the seven lanes by z so
  their lamp columns never increase (f2 e2 | a1 g2 d1 | c0 b0 by column), and a lane's west run
  then never crosses a farther lane's north run; within a column the rows already differ by 2, so
  no repeater stands on another's support. Pair the segments per band as {f,e}, {a,g}, {d,c}, {b}
  to keep every band's two rows within 2, which a 4-cell staircase in the lane can absorb.
