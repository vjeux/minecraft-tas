# HANDOFF: track shrink (2026-10-19)

## State
- Records (all archived on main by the coordinator unless noted; verified at 32 positions x 4 orientations):
  - **t7b-ob40-40b**: 63b (7x9x1), 40 blocks, solid, open 41 / close 29 (the overall solid record). BEST.txt.
  - **t7b-sk40-40b**: SCULK 63b, 40 blocks (ob40 + a sensor-driven left wall), 47 / 47.
  - **t7b-ob40e-41b**: the record-ELIGIBLE variant of ob40 (note block (1,5) under the lever),
    70b (7x10x1), 41 blocks (pushed 5e10899a; pinged; archive on main pending the coordinator).
  - **s27-tlp50-50b**: solid 8x8 (64b), 50 blocks (on pockets2's s27-trbl51).
  - t7b-pw42-42b, t7b-p42-42b (42), t7b-nb2-43b (43, 37/25).
- Tool: `mctas door fill` (crates/piston-door/src/fill.rs; CLI in crates/door-redsim), now owned by
  pockets3 (roster). Last commit of mine: e22d9902 `--require-any` (sculk variants).

## What is exhausted (t7b-ob40 -> 39, all with pistons in the palette)
Walls k<=2 (also sculk k<=3: only the 40 sculk), left 10 cells with D/gate free k<=5 (solid + sculk),
bottom-right k<=3 + watcher (solid + sculk k<=4), middle 16 cells k<=5 connected and --gaps.
So 39 needs a coupled change across regions or a new mechanism (NOTES.md has the timeline).

## Exact next steps
1. e78 (e78k's 7x8 sculk partial, partials/e78.md): r2.txt runs on box1 (runs/e78r2-6k4.log,
   e78r2-8k4.log, e78r2-6k5r.log; finds -> finds/e78r2-*). Report results to e78k (da291dc5).
   r1.txt regions done (0).
2. Apply `door fill --palette ...,sticky,piston --gaps` to any partial that becomes one input away.
3. A t7b-family door cannot be eligible in 63b (the lever's support is the T puller); an eligible
   63b needs another family.

## Files
- tracks/shrink/NOTES.md (results, how ob40 works, empty screens, t7b bottom windows), finds/ (all
  finds by run), runs/ (logs), plans/ (ideal plans of t7b's bottom), wip/ (bases), archive/,
  relever/ (eligible variants).
