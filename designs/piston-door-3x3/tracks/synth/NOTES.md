# Track synth: circuit synthesis by exhaustive simulation

Tool: `mctas door synth` (crates/piston-door/src/synth.rs). Given a context (a door design with
the sub-circuit to replace cleared, or a gadget harness), a box of free cells, a palette and a
target, it enumerates every circuit of 1..k blocks that can carry a signal from `--from` cells to
`--to` cells, screens each one in reused sandboxes on 24 JVMs and judges the survivors (the door
judge at 32 positions, or a `--spec` behavior script at 2 world offsets). Smallest sizes first.

* Enumeration = growth along a static signal model (`Node`/`drives`): every new block must be
  driven by a source, a circuit block or a context block the circuit powers (repeater back,
  dust next to a powered block / dust / component, torch on its block, observer watching a block
  that changes, `--watch` cells for moving parts); parts that need support get a floor
  (`--floors`, counted) unless one exists. Kept: some driven block drives a sink and (unless
  `--loose`) every block is on a source-to-sink path or holds one up. Visited sets are shared in
  a sharded table over 32 threads (14M sets in 4.5 s); a distance bound prunes sets that cannot
  reach a sink in the remaining budget; `--require kind:n` / `--limit kind:n`.
* Screening: ~2,300 door screens/s or ~4,000-8,000 spec runs/s on 24 workers.
* Bridge: `step` takes `signals: [[x,y,z]...]` and returns, per tick, the best signal each cell's
  neighbors give into it (what a piston/lamp there sees) and the strong signal into it
  (`Worker::step_probe`); op `signals` reads it now (`Worker::signals`, `door drive`'s
  `probe_signal`). Specs use it: `probe out x y z`, `use`, `run n`, `expect out 1..12 on`.

Tool: `mctas door resynth <design> --out F [--min-size 2 --max-size 4]`: large-neighborhood
search by exact synthesis. Every connected group of removable blocks (wiring, plain blocks that
do not move; never the door, lever + support, pistons, slime/honey, redstone blocks) is cleared
and every circuit of fewer blocks reconnecting its neighbors (sources = sinks = the context cells
around the group; observers may watch any cell that changes while the door cycles; nothing in
cells the machine sweeps) is screened and judged; an accepted replacement is applied and the
search restarts. Test: fc2 with P_M's block replaced by dust + floor (41) comes back to 40 in 18 s.
Size-4 groups take ~1 s, size-5 groups ~7 s (70-80k circuits each).

## Results

### Records from `door resynth` (verified --extra-offsets 28, also with main's new support check)
| file | blocks | board | from | what changed |
|---|---|---|---|---|
| `BEST-gravity.txt` = `gravity-resynth-42.txt` | **42** | gravity (record, was 45) | gravity-g10 45 | stone+dust+stone+repeater feed -> 2 observers + a delay-1 repeater; stone+dust+stone -> block + observer; a dust dropped. With the new feed a piston rests retracted, so the file is the settled state (open 5 / close 17) |
| `BEST-slime-free.txt` = `sf-resynth-46.txt` | 46 | solid, slime-free (record when found, was 48; fresh-d/s7q reached 45/44 since) | slime-free-combo 48 | stone+dust+stone+repeater -> block + 2 observers; observer + its floor -> 1 observer (open 24 / close 24) |

`door resynth` found nothing on fc2 40 (sizes 2..5, 115 + 136 groups, up to 128k circuits per
group), st43 sculk 42 (2..4), fresh-b mp8 43 (2..4), gravity 42 (2..5), slime-free s7q 44 (2..4).

### fc2 (40, the record) sub-circuits: all already minimal in this model
| sub-circuit (blocks) | cleared | searched | result |
|---|---|---|---|
| B window O_b+block+R_B (3) | (-1,5,0) (-1,6,0) (0,6,0) | k<=3, loose, 38 states, watch Q/RB/A/P_R cells | only the original + 2 variants (dust or dot on top of O_b instead of the block); no 2-block chain |
| P_L driver o+obs (2) | (-3,4,0) (-3,3,0) | k<=2, loose | only the original |
| P_M driver obs+block (2) | (2,-1,0) (2,-2,0) | k<=2, loose | the original + its mirror (observer under the bottom-LEFT door block) |
| A/B control O_a,X,X floor,O_b,block,R_B,O (7) | 7 cells | k<=6, strict, 38 states x 2 floors: 396k circuits | none |

### Problem 2: "lever on, plus >= 12 ticks after it turns off" with < 2 repeaters
Harness `designs/gadgets/synth/hold-harness.txt` (lever on X, output = the cell east of X, which X
already powers while the lever is on), spec `designs/gadgets/synth/hold12.spec` (output on at
ticks 1..12 after the off flip, off by 30, two cycles).
* With 2 repeaters the smallest hold is 7 blocks (4 found, e.g. R(0,1,1) reads X, block, dust,
  R(1,1,1) back into the output cell, 3 floors): gadget `designs/gadgets/hold-2rep` (+ drive test,
  using the new `probe_signal`).
* No hold of <= 6 blocks at all (40k circuits, palette blocks, dust (cross/dot), repeaters of
  every delay, observers, torches, comparators, lamps), and none of <= 7 blocks with at most one
  repeater (407k circuits). A one-repeater hold needs >= 8 blocks, or a mechanism outside the
  static model (BUD pistons, moving redstone blocks).

### The 47-record's problems (rb6-bmobs-47b)
* Problem 1, the 7-block silent hold for Q: cleared the 7 cells, searched every circuit of <= 6
  blocks driven by the lever or its block X_L that drives X_L's cell or Q's (palette block, glazed
  terracotta, dust, dot, repeaters 1-4, observers, torches, comparators; floors block/glazed
  terracotta; box x -4..1, y 3..7, z -4..0): 524k circuits, none passes the door screen. The
  7-block hold is minimal in this model.
* Problem 3, the O2 -> R_B bridge (4 blocks: block, cross dust, its floor, block): every circuit of
  <= 3 blocks from O2/the lever to R_B's input, B's quasi cell or B (watching Q/RB/A cells too):
  none; at 4 only the original and a glass-floor variant.
* (The coordinator then reported fc2 = 40 with no Q hold, which made problem 1 moot.)

## What did not work / limits
* The static model only grows circuits that a signal can traverse: blocks whose only job is to
  block a dust connection, or update relays (BUD), are not generated (use `--loose` for dangling
  blocks). Pistons are not circuit parts yet (a pulse-driven side piston can't be re-placed).
* Moving the lever (a port) is outside resynth: in fc2 the lever's own support block and R_q's
  floor are the most likely savings, and they need a lever move.
* A design whose replacement leaves a piston unpowered at rest settles into another arrangement
  (gravity 42): `canonicalize` keeps only the original positions and breaks it, so resynth now
  writes the settled state (every block read back after placement) when the canonical form fails.
* Early runs leaked a sandbox per synth call (resynth calls it hundreds of times) and a JVM ran
  out of memory; sandboxes are now created once per worker and reused, and a worker whose JVM
  dies gives its unscreened candidates back to the others.

## Next ideas
* Pistons and the lever as circuit atoms (pulse-driven pistons anywhere they can pull a block 2
  away; lever positions around the cluster), so resynth can move P_L/P_M and the lever.
* Size-5/6 resynth on the slime-free leaders (each size-5 group costs ~10-20 s; sf44 has ~150).
* A speed objective for resynth (keep open/close <= the start's) for the speed boards.
