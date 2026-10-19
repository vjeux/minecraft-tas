# clk-obs: block-update clocks (track notes)

Track clk-obs of the clock study ([docs/clocks.md](../../../../docs/clocks.md)): observers, pistons
and sticky pistons (moved blocks, moving observers), copper bulbs, note blocks, lamps,
trapdoors and their hybrids. Tools: `crates/clock-obs` (`mctas clock obs <cmd>`). Every period
below was measured, never assumed: redsim (`mctas clock sim`) for the searches, the real game
(`mctas clock verify`, 8 positions x 4 rotations) for every reported clock.

## Tools (`mctas clock obs ...`)

| command | what |
|---|---|
| `sim <designs> [--cells] [--trace N]` | this crate's fast redsim runner (period, family, waves); official numbers: `mctas clock sim` |
| `census --k K [--palette ...]` | EXHAUSTIVE: every assignment of palette states to every polycube of K cells (one shape per rotation class), each with every kick (self-start, each block placed `last`); keeps the MINIMAL clocks (no single block removable keeping the period), one per type |
| `grow <seeds> [--rounds N]` | local search: one block added (box + 1) or replaced, per seed; the new minimal clocks of a round seed the next |
| `rings --max-len N` | every pure observer ring (closed self-avoiding lattice loop) up to N observers |
| `rings --relay note,lamp,bulb,trap` | the same rings with one relay block instead of one observer |
| `rings --max-len N --pulses t1,t2,..` | experiment: a ring carrying several injected pulses (steady period) |
| `pick <folders> --out-dir D` | the smallest clock per (period, family), re-run with `clock::sim`, named `p<P>-<family>-<blocks>b.txt` |
| `cores <folders>` | groups clocks by their steady-state CORE (changing cells + the static blocks they need, up to rotation and phase): distinct machines vs starters |
| `divider <base> --bulbs K` | a copper bulb + comparator chain out of an observer's back: P = P0 x 2^K |

Palette shorthands: obs, piston, sticky (6 facings each), note, lamp, bulb, stone, rblock, glass,
slime, honey, obsidian, trap (iron trapdoors, 8 states).

## Results

See `smallest/` (<= 5 blocks, from the census) and `smallest6/` (6 blocks, new (period, family)
pairs from grow), each file headed by its `clock sim` summary; `logs/` holds the `clock verify`
runs. The board (Space memory `minecraft-tas/clocks/families/obs.md`) has the live tables.

### Census (exhaustive), palette obs/piston/sticky/note/lamp/bulb/stone/redstone block

| k | assignments x kicks | time | minimal clock structures | periods |
|---|---|---|---|---|
| 2 | 2.5 K sims | 0 s | 2 (the observer pair, horizontal and vertical) | 6 |
| 3 | 219 K | 0.1 s | 3 new (pair + bulb) | 6, 12 |
| 4 | 34.6 M | 6 s | 447 | 4, 6, 8, 10, 12, 13, 16, 18, 24 |
| 5 | 5.24 G | 18 min | 24 541 | 4, 6, 7, 8, 9, 10, 12, 13, 14, 15, 16, 17, 18, 20, 23, 24, 26, 27, 28, 48, 52 |
| 6 (grow from all 5-block clocks) | 36.9 M candidates | 8 min | 105 482 | adds 5, 11, 19, 21, 22, 30, 32, 35-38, 40, 42, 43, 46, 54, 60, 78 |

Distinct cores (k = 5): P4 4311 structures / 930 cores, P6 10796 / 201, P7 132 / 6, P8 627 / 455,
P12 4979 / 3724 (most P4/P6 structures are the observer pair with different starters).

### Theory

* P1 impossible (the end-of-tick state would be constant). P2 impossible here: every changing
  component has its own period >= 3 (observer >= 4: 2 gt pending + 2 gt on; lamp >= 4; piston >= 3,
  a 0-tick every 3 gt at best; bulbs, note blocks, trapdoors follow their power), and a clock's
  period is a common multiple of its components' periods.
* P3: no observer, lamp or bulb can change (periods >= 4 or even); only pistons 0-ticking every
  3 gt with redstone blocks and followers. None up to 5 blocks.
* Pure observer clocks: each observer watches one cell, so the watch graph is functional (one
  cycle per component) and lattice cycles have even length n. One pulse runs 2 gt per observer:
  P = 2n for n >= 4. The pair (n = 2) has two stable modes chosen by the order of the two
  observers' ticks in the handover tick (hidden state): P6 (how a placed pair starts) and P4
  anti-phase (needs a starter, e.g. a moving observer: it fires 2 gt after it lands). With one
  `last` kick a pure observer build reaches exactly P in {6, 8, 12, 16, 20, ...}.
* A note block (or iron trapdoor) relay in a ring follows its power with no delay but takes a
  lattice cell, so a ring of 5 observers + 1 note block has P = 10: P = 2 mod 4 (>= 10).
* A copper bulb halves the frequency (it toggles on rising edges; observers see both of its
  edges, so a comparator must read it to divide further): pair + bulb = P12 in 3 blocks;
  `divider`: P = 6 x 2^k with 3k blocks (k = 12: P = 24576 gt, 36 blocks).
* Odd periods come from pistons: a block landing in the block-entity phase makes a piston fire
  the NEXT tick (+1 gt). The 6-block P5 runs a 10 gt loop carrying two pulses 5 gt apart (the odd
  offset is made during the warmup).
