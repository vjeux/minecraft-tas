# Track ground: the first door on "Solid, on the ground (nothing below the door)"

Goal (coordinator, 2026-10-18 19:29Z): no block below the door's bottom row, at rest or while it
moves, and it must still work standing on a stone floor (`ground_check`: slime may not drag the
ground, nothing may push or power through it). All other rules apply (nothing in front/behind at
rest, rotation-safe). Start: shapes' `bases/ground20.txt` (20-block mechanism, works with ideal
inputs: `ground20.ideal` per edge, `ground-sym.ideal` one 24-pulse train for both edges).

## Log
- 19:30Z setup: clone, branch explore/ground, merged origin/explore/shapes (its NOTES, sym-bfs
  cell print, shape cache v2). `door ideal ground-sym.ideal --set s=5` WORKS (123/123 ticks).
- 19:40Z mech-wire on ground-sym (24 pulses): route2 waves are 64 ticks; widened to 128 (u128)
  locally: still 0 lever spots reach every receiver (long multi-pulse waves). Reverted.
- 20:10Z NEW MECHANISM `bases/shift2.txt` (21 blocks, slime-free, nothing below): per row, a
  sticky puller P pulls the side block out, a z-piston Z parks it at depth 1, a shifter Q pushes P
  into the ring so P pulls the middle block in, Q pulls P back, P pulls the middle block out; the
  far side block by a simple puller. One row solved by `mech-synth --sym-bfs` (bases/row-shift.txt):
  ONE 7-step train for both edges: P Z Q P Q P side. Rows are independent, so the 3x3 runs the same
  7 steps for all rows at once (rows 1-2 exit left, row 0 mirrored to the right: a shifted bottom
  piston can only be powered by quasi-connectivity through the row above's ring cell).
  `ideal/shift2.ideal` WORKS (s=6: 39/39 ticks; s=5: 33; s<=4 fails). Power cells chosen so stacked
  pistons are powered from above the top one (a row-1 power cell would quasi-power row 0's piston).
- 20:25Z coordinator: SEAMLESS required. shift2 is NOT seamless (Z pistons and the z-parked side
  blocks sit in the hallway ring S at depth +-1; any z-parking at x=-1/3 is in S at every depth).
  ground20 (elevator) IS seamless-compatible (open state = exactly the 9 door-plane ring cells,
  all its pistons outside S) but needs 24 pulses. Wiring not done.

## Next ideas
* Seamless rows: the side block must leave to x=-2 (beyond the ring) and the middle block to the
  ring; the conflict is that the middle block's second pull needs the puller at x=-2. Moving the
  puller or the side block in z at x=-2 (outside S) breaks the conflict at the cost of 1-2
  pistons per row; check with --sym-bfs (all rows in parallel keeps the train short).
* Wire shift2 anyway (non-seamless lead): 4 power groups per side, sources from above
  (-2,3,0) P home, (-1,3,0) P shifted, (-1,3,-1) Z, (-3,3,0) Q, (-3,0,0) LP0; right: (4,0,-1),
  (3,1,-1) (+glass door blocks or the shifted P's QC power makes R1 power RP1), (3,1,1)/(3,0,2),
  (5,1,0), (4,3,0). A 7-step observer/repeater chain from one lever.
