# Track router2: a port-aware, timing-aware net router (`door route2`)

Goal: infrastructure that shrinks wiring. The old router (`door route`) lays plain dust between
two cells; `door route2` routes whole nets: it taps any powered block (or watches any cell the
machine changes), enters each receiver any way the game allows, places dust, repeaters (any
delay), observers and conductor blocks, and makes every receiver get its signal tick for tick.
Then it rips up and re-routes every connected group of control blocks of a design with fewer
blocks. Generic: any design with a `lever` port (a door port only adds the door/walkway
exclusion and the judge). Code: `crates/piston-door/src/route2.rs`.

## How it works

1. **Measure** one open/close cycle of the design in the simulator (`measure`): the cells that
   hold a receiver at some tick (pistons; dispensers/droppers/crafters, which are also
   quasi-connected; lamps, note blocks, doors, trapdoors, rails, TNT, bulbs), each with an
   occupancy mask; the cells the machine moves through (no new block there); the cells next to
   slime at some tick (only glazed terracotta there); the waves of sources the machine drives
   (observers watching moving blocks, shuttled redstone blocks, sculk sensors, components that
   ride the machine); and, for every cell the machine changes, what an observer watching it
   would emit (watch taps).
2. **Static model** (`Model`): every block is a node with a `Wave` (on/off at each tick of one
   open and one close phase, 64 ticks each). Feeds follow the game: dust shape from
   `getConnectionState` (dust powers only where it points), weak vs strong power (a block powered
   by dust powers pistons/repeaters but not dust), connected dust nets as units,
   quasi-connectivity (only from emitters whose own change updates the piston), repeaters (delay,
   pulse stretching, locking), observers (2 gt pulse 2 gt after a change, ignoring changes while
   powered), torches, lamps (4 gt fall). The state machines mirror `DiodeBlock`,
   `ObserverBlock`, `RedstoneTorchBlock`, `RedstoneLampBlock`.
3. **Requirements** come from the original: every receiver cell must get the same wave on the
   ticks a piston is there (plus the tick before it lands), except power a retracted piston gets
   while it stays retracted (blocked: changes nothing); components that ride the machine must
   keep their inputs. One check covers required receivers, timing windows and cells that must
   stay unpowered. `--tolerance N` accepts a cell's wave shifted by up to N ticks.
4. **Search** (`route_net`): the receivers a rip broke are connected one at a time (a beam over
   orders and alternative hits) by a Dijkstra over placements (cost = new blocks incl. floors)
   from every tap at once (any powered block, or an observer on any cell the machine changes).
   Each placement's wave is computed from all its inputs (dust strength tracked); a placement
   that would power any receiver outside its wanted ticks is pruned. Existing powered blocks are
   free relays; new plain blocks are conductors.
5. **Rip-up**: every connected group of 2..=`--max-rip` control blocks (`--pairs`: also two
   separate groups, i.e. two nets re-routed together, which is how a shared source would show
   up), closed under support (no kept block loses its floor or wall). Candidates the model
   accepts go to search.rs `try_edits` (screen at 2 positions, judge at 32), then canonicalize +
   re-verify in a fresh sandbox. `--polish`: equal-size re-routes are judged too and each gets a
   short local search (a plateau the minimizer's own moves cannot reach).

Usage:
```
mctas door route2 <design> --out <file> [--max-rip 7] [--pairs] [--tolerance N] [--archive]
mctas door route2 <design> --out <file> --model-only          # waves + calibration vs the sim
mctas door route2 <design> --out <file> --need='x,y,z=pulse+4/2;...'   # route a new net
mctas door route2 <design> --out <file> --polish --moves singles,pairs,swap
```
`--need` waves: `level+D`, `pulse+D/L`, `on`, `off`, `open:A-B;close:C-` (ticks after each flip).
Speed designs: pass `--max-open/--max-close` to keep their timings at the judge.

## Sanity checks
- fc2 with `--extra-blocks 1` (equal-size re-routes allowed): the router re-derives each of
  fc2's four control nets exactly (R_q + floor; O_m + its block; P_L's two observers; the B
  window O_b, J, R_B, with dust instead of J).
- `--need`: fc2 without P_L's two observers, asked for `-3,2,0=pulse+4/2`: it finds the same two
  observers and the judge passes the result at 32 positions (40 blocks).
- Generic (no door port): `designs/gadgets/route2-lamp.txt --need='4,1,0=level+4'` (a lamp that
  follows the lever 4 gt later): 5 blocks (dust, a plain block as a conductor, a delay-2
  repeater, two floors).

## Model fixes found by the calibration (each made the model match the simulator)
- Bidirectional dust-dust feeds latch in a fixpoint (g10's torch never turned its dust off):
  connected dust nets are evaluated as one unit fed by their external inputs.
- Observers dragged by slime (g10's O, g42's O) and other moving components: measured waves,
  and a re-route must keep their inputs (g42: 9 false candidates).
- Repeater locking (w6's locked repeater): a diode into a repeater's side freezes it; the state
  at rest is the cyclic steady state.
- Quasi-connectivity from a powered block diagonal to a piston goes unnoticed in the game
  (nothing updates the piston two cells away; st43's 38 model-accepted candidates all failed the
  screen for that reason): QC counts only from emitters whose change updates the piston.
- Lamps go dark 4 gt after losing power.

Calibration now (every control component, every tick of a cycle): fc2 10/10, mp8 9/9, s7 16/16,
s7q 17/17, st43 8/8, g10 10/10, synth-g42 10/10, speed w6-109b 48/48.

## Results
| design | rip sets routed (support-closed) | cheaper routings | model-accepted | judge | result |
|---|---|---|---|---|---|
| fc2 40 | 537 (max 7, pairs); 1388 (max 8, pairs, beam 10); 881 at tolerance 2 | 0 | 0 | - | local minimum |
| mp8 43 | 145 (max 8, pairs) | 0 | 0 | - | local minimum |
| s7 45 | 283 (max 6) + 1825 of 3735 (max 6, pairs) | 0 | 0 | - | local minimum so far |
| s7q 44 | (see log) | | | | |
| st43 42 | 397 (max 7, pairs); 492 at tolerance 2 | 0 (38 before the QC fix) | 0 | - | local minimum |
| g10 45 | 1454 (max 7, pairs); 3646 at tolerance 2 | 0 | 0 | - | local minimum |
| synth-g42 42 | 674 (max 7, pairs) | 9 | 0 (after the mover fix) | 0 of 4 before it | local minimum |
| speed w6 109 (open 12 / close 7 kept) | 543 (max 6) | 685 | 589 | 0 of 482 pass the screen | the model misses a trick (below) |

The records are local minima for whole-net re-routing too: up to 7-8 ripped control blocks,
two nets at once, any taps (including observers on any moving block), any entries,
dust/repeaters/observers/conductor blocks, exact timing on the ticks each piston is there.
Their remaining slack is in the mechanism, not in the wiring.

**w6-109b**: its locked repeater g (1,6,-1) only drives an isolated stone (1,6,0), yet removing
its lock (torch I, repeater i and its floor: "rip -> nothing", the model sees no receiver
change) leaves the door shut. The trick is outside the model (most likely block updates: a
BUD), so the simulator rejects all of w6's candidates, as it should.

## Next ideas
- Model block updates (who updates whom, BUD-powered pistons): w6's trick and any design that
  uses update order.
- Behavioral equivalence beyond blocked ticks: an extended piston ignores extra power, so only
  its falling edge matters; a mechanism-level simulator check per receiver would accept more.
- Use `route_new` as the wiring oracle of mechanism synthesis: for each candidate mechanism
  (ideal input schedule), the router gives the cheapest wiring that produces the schedule, so
  mechanisms can be ranked by total blocks before anyone wires them by hand.
- Move the lever (port) as part of the routing; comparators in the palette.

## Log
- 02:50 route2 module (static model, router, rip-up), sanity re-derivations on fc2.
- 03:00-04:10 runs on the records; each calibration miss became a model fix (above).
