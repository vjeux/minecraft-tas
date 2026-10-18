# Track router2: a port-aware, timing-aware net router (`door route2`)

Goal: infrastructure that shrinks wiring. The old router (`door route`) lays plain dust between
two cells; `door route2` routes whole nets: it taps any powered block (or watches any cell the
machine changes), enters each receiver any way the game allows, places dust, repeaters (any
delay), observers and conductor blocks, and makes every receiver get its signal tick for tick.
Then it rips up and re-routes every connected group of control blocks of a design with fewer
blocks. Code: `crates/piston-door/src/route2.rs`.

## How it works

1. **Measure** one open/close cycle of the design in the simulator (`measure`): the cells that
   hold a piston at some tick (receiver cells, with their facing and an occupancy mask), the
   cells the machine occupies or moves through (no new block there), the cells next to slime at
   some tick (only glazed terracotta there), the waves of sources the machine drives (observers
   watching moving blocks, shuttled redstone blocks, sculk sensors), and, for every cell the
   machine changes, what an observer watching it would emit.
2. **Static model** (`Model`): every block is a node with a `Wave` (on/off at each tick of one
   open and one close phase, 64 ticks each). Feeds follow the game: dust shape from
   `getConnectionState` (dust powers only where it points), weak vs strong power (a block powered
   by dust powers pistons/repeaters but not dust), quasi-connectivity (a piston also reads what
   the neighbors of the cell above it emit into it), repeaters (delay, pulse stretching),
   observers (2 gt pulse 2 gt after a change, ignoring changes while powered), torches. The
   element state machines mirror `DiodeBlock`, `ObserverBlock` and `RedstoneTorchBlock`.
   **Calibration**: on fc2 (10 components), mp8 (9), s7 (16) the model matches the simulator on
   every control component, every tick (`--model-only` prints it).
3. **Requirements** come from the original: every receiver cell must get the same wave on the
   ticks a piston is there (plus the tick before it lands). One check covers required receivers,
   timing windows and cells that must stay unpowered.
4. **Search** (`route_net`): the receivers a rip broke are connected one at a time (a beam over
   orders and alternative hits) by a Dijkstra over placements (cost = new blocks incl. floors)
   from every tap at once. Each placement's wave is computed from all its inputs; a placement
   that would power any piston cell outside its wanted ticks is pruned.
5. **Rip-up**: every connected group of 2..=`--max-rip` control blocks (`--pairs`: also two
   separate groups, i.e. two nets re-routed together, which is how a shared source would show
   up), closed under support. Candidates the model accepts go to search.rs `try_edits` (screen at
   2 positions, judge at 32), then canonicalize + re-verify in a fresh sandbox.

Usage:
```
mctas door route2 <design> --out <file> [--max-rip 7] [--pairs] [--tolerance N] [--archive]
mctas door route2 <design> --out <file> --model-only          # print waves + calibration
mctas door route2 <design> --out <file> --need='x,y,z=pulse+4/2;...'   # route a new net
```
`--need` waves: `level+D`, `pulse+D/L`, `on`, `off`, `open:A-B;close:C-`.

## Sanity checks
- fc2 with `--extra-blocks 1` (equal-size re-routes allowed): the router re-derives each of
  fc2's four control nets exactly (R_q + floor; O_m + its block; P_L's two observers; the B
  window O_b, J, R_B, with dust instead of J).
- `--need`: fc2 without P_L's two observers, asked for `-3,2,0=pulse+4/2`: it finds the same two
  observers and the judge passes the result at 32 positions (40 blocks).

## Model fixes found by the calibration (each made the model match the simulator)
- Bidirectional dust-dust feeds latch in a fixpoint (g10's torch never turned its dust off):
  connected dust nets are now evaluated as one unit fed by their external inputs.
- Observers dragged by slime (g10's O) and other moving components: measured waves.
- Repeater locking (w6's locked repeater): a diode into a repeater's side freezes it; the state
  at rest is the cyclic steady state.
- Quasi-connectivity from a powered block diagonal to a piston goes unnoticed in the game
  (nothing updates the piston two cells away; st43's 38 model-accepted candidates all failed the
  screen for that reason): QC counts only from emitters whose change updates the piston.
- Lamps go dark 4 gt after losing power.

Calibration now (every control component, every tick of a cycle): fc2 10/10, mp8 9/9, s7 16/16,
st43 8/8, g10 10/10, speed w6-109b 48/48.

## Results
| design | rip sets | cheaper routings | model-accepted | judge | result |
|---|---|---|---|---|---|
| fc2 40 | 1163 (max 7, pairs; 537 support-closed) | 0 | 0 | - | local minimum |
| mp8 43 | 333 (max 8, pairs; 145 support-closed) | 0 | 0 | - | local minimum |
| s7 45 | 3735 (max 6, pairs; stopped at 1825) + 283 (max 6) | 0 | 0 | - | local minimum so far |
| st43 42 | 586 (max 7, pairs; 397 support-closed) | 0 (38 before the QC fix, all screen failures) | 0 | - | local minimum |
| g10 45 | 5875 (max 7, pairs; 1454 support-closed) | 0 | 0 | - | local minimum |
| fc2 40 (deep) | 1388 (max 8, pairs, beam 10, 6 hits) | 0 | 0 | - | local minimum |
| speed w6 109 (open 12 / close 7 kept) | 4180 (max 6; 543 support-closed) | 685 | 589 | 0 of 482 pass the screen | the model misses a trick (below) |

The records are local minima for whole-net re-routing too (up to 7-8 ripped control blocks,
two nets at once, any taps, any entries, dust/repeaters/observers/conductor blocks, exact
timing on the ticks a piston is at each cell).

**w6-109b**: its locked repeater g (1,6,-1) only drives an isolated stone (1,6,0), yet removing
its lock (torch I, repeater i and its floor: "rip -> nothing", the model sees no piston cell
change) leaves the door shut. The trick is outside the model (most likely block updates: a
BUD), so the simulator rejects all of w6's candidates, as it should.

## Log
