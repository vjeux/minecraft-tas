# Track pistmodel: the complete, game-verified piston / moving-block model

Brief (vjeux via the coordinator, 2026-10-18 22:25Z): "The remaining ones are truly hard and will use
all the redstone capabilities. So we need proper models of everything we can use, not just simple
stuff." The judge runs the real game, but the mechanism search (`mechsynth.rs`, `families.rs`) used a
simplified macro model (in-plane pistons, no helpers, limited pulses). This track built the exact
tick-level model of pistons and moving blocks in Rust (`crates/redsim`, shared with track redmodel's
redstone model), fuzzed it against the real game, then put the mechanism search on it.

## 1. Inventory
[INVENTORY.md](INVENTORY.md): everything pistons do, read in the 26.3 source (tick phases, block
events, structure resolver, moving block entities, spitting / 0-ticks, push reactions, slime/honey,
falling blocks, exclusions).

## 2. The model (`crates/redsim/src/piston/`, `crates/redsim/src/gravity.rs`)
* `base.rs` (`PistonBaseBlock`: power with quasi-connectivity, `checkIfExtend` with the event 2 rule,
  `triggerEvent`, `moveBlocks` with the Java `HashMap` order of vacated cells), `resolver.rs`
  (`PistonStructureResolver`), `head.rs`, `moving.rs` (`PistonMovingBlockEntity` + the block-entity
  map and ticker list with rebinding), `shape.rs` (collision shapes and `isFaceSturdy` FULL / CENTER /
  RIGID of bases, heads and moving pistons), `gravity.rs` (falling blocks, vertical falls).
* Ideal inputs for the mechanism search: tracked per piston (a carried piston keeps its input); the
  source sits behind the piston's cell when switched on, powers it and, by quasi-connectivity, the
  piston right below (exactly what a `door ideal` redstone block at z = 1 does).

## 3. Differential fuzzing: `mctas door pistfuzz`
Every block state, every moving block entity (moved state, direction, extending, source, progress),
the block-event / scheduled-tick / moving-piston counts, drops and the idle flag compared after every
tick between the bridge (the real game) and redsim.

| run | result |
|---|---|
| random piston setups (7x7x1 flat / 6x6x3 at random world positions; pistons of every facing, extended at start, stone, obsidian, crying obsidian, glazed terracotta, redstone blocks, observers, slime, honey; redstone blocks set / cleared incl. 0-5 gt pulses, pistons placed mid-run) | **100,000 / 100,000 identical** (first run 179/200: one engine bug) |
| mixed palette (dust, torches, wall torches, repeaters, comparators, levers clicked, lamps, note blocks, targets next to the pistons) | **97,541 / 97,541 identical** of 100,000 (seed 7000000, 4.68M ticks; 2,459 skipped: a click on a non-lever, an input redsim refuses) |
| gadget library (designs/gadgets/*.drive: judge placement, the script's inputs, every tick) | 72 / 72 (entity and sculk gadgets skipped) |
| archived doors (archive/*.txt: judge placement, 2 open/close cycles, every tick) | 328 / 328 (sculk, copper bulb, entity designs skipped) |
| gravity (sand, gravel, red sand with the piston palette) | 2975 / 3000 (seed 4000000): falling entities pushed by moving pistons (`moveCollidedEntities`) are not modeled |
| shaped blocks next to pistons (`--shapes`: slabs, trapdoors, scaffolding, fence gates; collision boxes from redmodel's `collision_boxes`) | 2988 / 3000 (seed 9100000): the rest are falling scaffolds pushed by moving heads (same gap) |

Bugs found and fixed: popped piston heads drop nothing (engine `destroy_block`, redmodel's); the
shape-update flags of `setBlock` clear NEIGHBORS (`& -34`, redmodel's, found as a BUD-fired piston
under a note block); moving-piston face sturdiness (a retracting base keeps its extended box: a
lever / repeater on its back survives: the only archived-door mismatch). Sensitivity checked by
mutation (disabling the sticky spit finalTick: 41/500 mismatches).

## 4. Mechanism search on the exact model: `mctas door fullmech`
Every transition is the real game (redsim): spitting vs pulling decided by the tick, 0-ticks and the
block-event order, carried pistons landing and firing, carried / static redstone blocks and
observers, quasi-connectivity and BUD. Actions: k-tick pulses (1 = spit, 3 = push-pull), level
toggles (pistons extended at rest, or all with --levels), overlapping pulse pairs (--pairs: same-tick
and next-tick timing). FULL seamless open + exact close back to rest.

Families' candidates re-solved exactly (<= 10 pistons, pulses 1/3, cap 30k states;
[fullmech/families-exact.txt](fullmech/families-exact.txt)):

| box | candidates | open + close exactly | not solved by the macro model (20k cap) | capped |
|---|---|---|---|---|
| 7x7 | 3072 | 0 | 0 | 1408 |
| 7x8 | 3448 | 32 | 8 | 1673 |
| 8x7 | 3424 | 32 | 8 | 1669 |
| 8x8 | 3800 | 64 | 16 | 1977 |
| 9x6, 6x9 | 32 each | 0 | 0 | 0 |
| 11x5, 12x5 | 0 (families' generator needs rows mode) | - | - | - |

* 7x7 with same-tick pulse pairs (600 candidates): 0.
* The 32 exact 7x8 mechanisms are exported with exact `door ideal` plans:
  [fullmech/7x8/](fullmech/7x8/) (INDEX.tsv); 32/32 replay in the real simulator
  (`door fullmech --verify-ideal`).
* With physical inputs, stacked pistons share a source: e89b's mechanism solves with 8 of its 10
  inputs.
* Input reduction on the 32 7x8 mechanisms ([fullmech/reduce-7x8.txt](fullmech/reduce-7x8.txt)):
  greedy undriving leaves 8 (24 mechanisms) or 9 (8) of 10 pistons with an input; no single helper
  (a redstone block, or an observer of each in-plane facing, in any free cell) lowers it.
* **Input merging** (`--groups`: pistons switched together by one shared signal): every one of the
  32 7x8 mechanisms needs only **3 independent signals**: one group of 6-7 pullers fired together
  (1-tick and 3-tick pulses) plus the two pistons of the center's elevator (or one of them and an
  extended-at-rest piston). Plans: [fullmech/7x8/GROUPS.tsv](fullmech/7x8/GROUPS.tsv) and
  `fm-7x8-NNN-groups.ideal`; 32/32 replay in the real simulator. e89b's mechanism: the same 3
  (6 pullers | SU(1,-3) | SU(1,-2)). A wiring therefore needs 3 timed signals, but the big
  group's pullers sit on both sides of the door (row y=1 is full at rest), so one network cannot
  reach them all in the plane: the signal has to cross through moving blocks or be duplicated.
* `--anneal` (simulated annealing over mechanisms not tied to exit plans, 15k evaluations in 90 s on
  7x8): nothing from scratch; it needs seeding.
* Groups + one helper (`--groups --reduce` on a design: a redstone block or an observer E/W/U/D in
  each free cell next to a piston or its QC cell): on all 32 7x8 mechanisms no placement brings
  the 3 signals down to 2.
* Piston removal + one helper (`--shrink`: drop each piston, add stone / obsidian / a redstone
  block / an observer in each free cell): on all 32 7x8 mechanisms no working variant with 9
  pistons.
* 8x8 (64 mechanisms) and 8x7 (32) exported the same way ([fullmech/8x8/](fullmech/8x8/),
  [fullmech/8x7/](fullmech/8x7/)): every one needs 3 signals; 192/192 plans replay in the real
  simulator.

## Log
* 22:33Z inventory; 23:02Z pistons in, 2000/2000; 23:11Z 100k/100k; 23:25Z gadgets + archived doors
  + mixed; 23:38Z `door fullmech`; 00:03Z families' boxes re-solved; 00:26Z 7x8 export verified.
