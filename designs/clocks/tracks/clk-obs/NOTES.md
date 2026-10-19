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
| `every --min A --max B [--out-dir D]` | one construction per period (observer ring, note ring, odd piston stage + tower), each confirmed by redsim |

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
  Necessary structure (2026-10-19): a piston of period 3 must 0-tick (extend at t, retract at
  t' >= t, extend again >= t' + 3, so t' = t); its own push cannot queue its retract (moveBlocks
  updates the arm cell's neighbors, the base included, BEFORE triggerEvent marks the base
  EXTENDED, and a retract is only queued for an extended piston), so a second piston's move must
  update it, after a power
  element was moved away in the same block-event phase. Power elements: redstone blocks, or a
  conductor next to a permanently powered observer. A sticky 0-tick places the block it pushed
  one cell on (it cannot pull it back) and a normal one leaves it moving, so every element moved
  away must be moved back by a DIFFERENT piston each cycle. So: at least 2 pistons and 1 element,
  each element with two distinct movers per cycle, every piston 0-ticking every 3 gt. (A counting
  bound beyond that is not proved: a piston can push the element that quasi-powers it when the
  element is second in its upward line.)
  Exhaustive targeted searches (`census --max-period 3 --min-pistons --min-rblocks`): no clock
  with P <= 3 among every <= 5-block polycube (the full palette, and piston/sticky/redstone
  block/slime/stone), and every 6-block polycube with exactly 4 pistons + 2 redstone blocks
  (1.94 G sims, 42 min); box mode (`--box WxHxD`, non-connected placements) next.
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

### Every period P >= 4 (constructions, `mctas clock obs every --min 8 --max 200`)

Every P from 4 up is reachable by block-update clocks: the census gives P4 (4 blocks), P5 (6),
P6 (2), P7 (5), P9 (5), and three parametric families give every P >= 8 (all 192 designs for
8..200 confirmed by redsim; the gaps of the census verified in the game, `logs/`):

| P | construction | blocks |
|---|---|---|
| 0 mod 4, >= 8 | ring of P/2 observers on a 2 x P/4 rectangle | P/2 |
| 2 mod 4, >= 10 | ring of P/2 observers + 1 note block (it follows its power at once but takes a lattice cell, so an odd number of observers closes the loop) | P/2 + 1 |
| odd, >= 11 | THE ODD PISTON STAGE + an observer tower ([oddring11](oddring11.txt), [oddring13](oddring13.txt)) | about P/2 |

The odd stage: observer O1 (fed by the ring) quasi-powers a sticky piston S (O1 touches the cell
above S, facing away); O2 watches O1 and powers S directly from above, so S stays powered 4 gt
(O2 turns on in the very tick O1 turns off) and does NOT spit its redstone block: the block LANDS
under a normal piston Q in the block-entity phase 2 gt after the push, so Q fires the next tick
(+1 gt) and its head enters the cell observer J watches: J fires 5 gt after O1. Q retracts when S
pulls the block back (4 gt). A tower of observers carries J's pulse back to O1, 2 gt each. Variant A
pushes the redstone block itself (J sits over an odd cell, the tower has an even number of
observers: P = 4h - 1 for a tower of height h); variant B pushes a slime block that drags the
redstone block one cell further (an odd tower: P = 4h + 1). A single 2 gt observer pulse is NOT
enough: the sticky piston would retract while the block is still moving and place it at once
(spitting), in the block-event phase, and Q would fire in the same tick (even again).
