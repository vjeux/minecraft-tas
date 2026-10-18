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
