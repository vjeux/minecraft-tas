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

* PRECISE: a piston push sets the pushed coordinate exactly (face + 0.01 + half the box) and a wall
  collision zeroes that motion, so pushing the primed TNT against a wall on both horizontal axes
  after its random kick makes it exact. Not built yet.
* BURST: two dusts beside the same redstone block lose their connection in the same push (each
  powers its own TNT); search running from the 12b (`search --require tnt,dust,fan`).
* MOVABLE: no silent power source rides a piston (dust pops; an observer lands unpowered unless an
  observer tick is pending at its landing cell, and the ticks observers schedule there are gone by
  then): no movable duper found or expected in 26.3.
* Lever on/off, slime-free (honey: a floor fan cannot stand on honey, its top is not sturdy).
