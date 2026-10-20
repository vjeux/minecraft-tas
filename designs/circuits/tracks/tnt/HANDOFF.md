# Track tnt: handoff

State (2026-10-20): study 9 (TNT dupers) has its judge, two archived designs and a catalog page.
Branch explore/tnt (merged into main up to e9a8f249e; later commits: archive, catalog, site,
tools). Notes: [NOTES.md](NOTES.md). Variations and results:
[designs/circuits/tnt/VARIATIONS.md](../../tnt/VARIATIONS.md).

## Archived (designs/circuits/tnt/archive/)

* tnt-duper-12b: self-running, 12 blocks, 200 TNT/min (period 6), robust; tileable (4 copies:
  800 TNT/min, robust).
* tnt-duper-triggered-13b: 1 TNT per input pulse, latency 1 gt, robust.

## Tools (mctas circuit tnt ...)

* `sim` (redsim; `--tile N --offset dx,dy,dz` for side-by-side copies), `run` (the real game
  with redsim's line next to it), `tile` (writes a tiled design for `circuit verify`).
* `search` (seeded random search: k palette blocks added to a seed in a box, required kinds placed
  first with their supports), `shrink`, `improve` (local search, the Pareto front of blocks vs
  TNT per minute; triggered designs scored by the judge's redsim check).
* `precision` (the real game: each primed TNT followed to fuse 1: spreads, direction class).

## Open variations, with what the code says

* PRECISE: from `PistonMovingBlockEntity.moveCollidedEntities`: a moving block pushes the entities in
  the slab it moves into this tick, by `min(needed, progress step) + 0.01` (step 0.5): an entity
  it overlaps only partly (a block ENTERING the entity's cell from one side) ends exactly at the
  face + 0.01, whatever its random offset; one it overlaps fully keeps its offset (moved 0.51).
  A push that ends against a wall also zeroes that motion (TNT has no bounciness), and a moving
  slime sets the motion on its axis to +-1. So an exact aligner = a block entering the TNT's cell
  from one side along z, then along x, each ending against a wall; then the fall is exact (its
  vertical motion is not random). In the 12b the TNT instead stays in the machine for its 80 gt
  fuse (pushed by the frame about half the cycles, bouncing on the slime): an aligner stage needs
  the TNT to leave the frame first. redsim has no piston-entity pushing yet (needed to search
  aligners on redsim with fixed kicks).
* BURST: one push priming 2 TNT does not fit: a second dust beside the same redstone block, its
  TNT and its fan push the frame past the 12-block limit, and the free placements sit in the
  frame's path (12.8M redsim samples from the 12b: none). The tiled 12b primes 4 TNT in the same
  tick (4 engines).
* The catalog's dominance fold (clk-rt/shulker's `circuit::archive::beats_on_variation`, on
  explore/shulker c548b39e9): once in main, add tnt-duper-triggered's lower latency / gap there
  so a bigger but faster-firing triggered duper keeps its own card.
* MOVABLE: no silent power source rides a piston (dust pops; an observer lands unpowered unless an
  observer tick is pending at its landing cell, and the ticks observers schedule there are gone by
  then): no movable duper found or expected in 26.3.
* Lever on/off, slime-free (honey: a floor fan cannot stand on honey, its top is not sturdy).
