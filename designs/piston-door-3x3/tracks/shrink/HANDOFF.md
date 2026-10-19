# HANDOFF: track shrink (2026-10-19)

## State
- Records (all archived on main by the coordinator, verified at 32 positions x 4 orientations):
  - **t7b-ob40-40b**: 63b (7x9x1), 40 blocks, solid, open 41 / close 29 (the overall solid record). BEST.txt.
  - **t7b-sk40-40b**: SCULK 63b, 40 blocks (ob40 + a sensor-driven left wall), 47 / 47.
  - **t7b-ob40e-41b**: the record-ELIGIBLE variant of ob40 (note block (1,5) under the lever),
    70b (7x10x1), 41 blocks.
  - **s27-tlp50-50b**: solid 8x8 (64b), 50 blocks (on pockets2's s27-trbl51); pockets2 stacked a
    sculk bottom-left on it: s27-bls49 (sculk 8x8, 49).
  - t7b-pw42-42b, t7b-p42-42b (42), t7b-nb2-43b (43, 37/25).
- Tool: `mctas door fill` (crates/piston-door/src/fill.rs; CLI in crates/door-redsim), now owned by
  pockets3 (board module table): send fill.rs requests to pockets3. My last tool commit: e22d9902
  `--require-any` (sculk variants of a solid door). archive/ is the coordinator's: keep captions in
  this folder (NOTES.md) and let the coordinator write archive/captions.tsv.

## What is exhausted (t7b-ob40 -> 39, all with sticky/normal pistons in the palette)
Walls k<=2 (sculk k<=3 gives only the 40 sculk); left 10 cells with D/gate free k<=5 (solid and
sculk); bottom-right k<=3 + BR watcher (solid; sculk k<=4); middle 16 cells k<=5 (connected, and
--gaps with a reduced palette); coupled bottom-right + (0,-4) (-1,-4) k<=5 + watcher (reduced
palette). So 39 needs a coupled change across more regions or a new mechanism.

## e78 help (e78k's 7x8 sculk partial, partials/e78.md): all 0
e78wsT right cells + (-2,-1) k<=4, 7 free cells k<=4; r1 left 5 cells k<=5, + top 2 k<=4; r2 6 cells
k<=4, 8 cells k<=4; nb 7 cells k<=4 (full palette) and k=5 (reduced); r1 right cluster (relay
included) k<=5, and + (2,-3) k<=5. Logs: runs/e78*.log. e78k handed off at 15:03Z (explore/q78
tracks/q78/HANDOFF.md); the results are on partials/e78.md.

## Next ideas
1. Pistons as wiring everywhere (a normal piston as a 1-block watched relay; a sticky piston moving
   an observer = merger + router): re-screen other designs' regions with them (docs/mechanics.md
   section 8 has the three tricks).
2. A t7b-family door cannot be eligible inside 63b (the lever's support is the T puller); an
   eligible 63b needs another family.
3. For a 7x8 solid t7b: B's 8 gt hold has no repeater support in 8 rows; a moving redstone block
   (sticky piston shuttle) or the moving-observer trick for the gates is the untested direction.

## Running processes
None (all of this track's door fill jobs finished; box1 devvm55865).

## Files
- tracks/shrink/NOTES.md (results, how ob40 works, empty screens, t7b bottom windows), finds/ (all
  finds by run), runs/ (logs), plans/ (ideal plans of t7b's bottom), wip/ (bases incl. e78 r1/r2/nb),
  archive/, relever/ (eligible variants).
