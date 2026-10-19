# HANDOFF: track shrink (2026-10-19)

## State
- Records (all archived on main by the coordinator, verified at 32 positions x 4 orientations):
  - **t7b-ob40-40b**: 63b (7x9x1), 40 blocks, solid (the overall solid record). BEST.txt.
  - t7b-pw42-42b, t7b-p42-42b (42), t7b-nb2-43b (43, 37/25).
  - **s27-tlp50-50b**: solid 8x8 (64b), 50 blocks (on pockets2's s27-trbl51).
  - **t7b-sk40-40b**: SCULK 63b, 40 blocks (ob40 + a sensor-driven left wall).
- Tool: `mctas door fill` (crates/piston-door/src/fill.rs; CLI in crates/door-redsim): exhaustive
  region filling judged as a real door on redsim; see NOTES.md. Merged on main up to --require-any
  (e22d9902 may still need merging).

## Exact next steps
1. t7b-ob40 -> 39: screens still open (each a single `door fill` command, see runs/*.log for the
   exact command lines' parameters):
   - left region with D/gate FREE + pistons, --gaps, k<=5 (runs/ob-left5.log): running/finished?
   - same with sculk + --require-any sculk (a sculk 39): queued.
   - middle region with sculk, k<=5: queued.
   - whole bottom minus the left chain with pistons + --gaps at k<=4 (right side 5 -> 4): not run.
2. Apply `door fill --palette ...,sticky,piston --gaps` to every live partial on the board
   (pistons as relays / moving observers are the trick that keeps paying off).
3. The relever (record-eligible) variant of t7b-type doors puts a note block at (1,5): the volume
   becomes 7x10 (runs/relever-p42.log); an eligible 63b needs the lever moved (open).

## Running processes (box1 devvm55865, started by this track)
- `mctas door fill` jobs writing runs/*.log (check `pgrep -af "door fill"`); safe to kill: only
  this track's.

## Files
- tracks/shrink/NOTES.md (results, how ob40 works, empty screens, windows), finds/ (all finds by
  run), runs/ (logs), plans/ (ideal plans of t7b's bottom), wip/ (bases), archive/.
