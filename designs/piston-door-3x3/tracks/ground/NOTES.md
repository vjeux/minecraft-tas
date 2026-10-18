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
- 20:37Z tool: `mech-synth --seamless` (macro model: nothing ever enters the hallway ring S off
  the door plane, no candidate there, the open state holds only door blocks in S). On one row
  (`bases/row-seam.txt`, `--add 3 --lo=-6,0,-2 --hi=6,0,2 --kinds sticky,normal --pulses-only`):
  seamless row mechanisms with a 10-step symmetric train, e.g. Q (-3,0,0) E, Zs (-2,0,-2) S,
  Zn (-2,0,2) N: SYM = P Zs Zn Q Zs Q P Q P RP (the puller P itself is moved in z at x=-2, outside
  S, so the side block can pass the ring and park at x=-2). Open: side block at x=-2, middle block
  in the ring x=-1, far block in the ring x=3.
  Open problem for the 3x3: the BOTTOM row's shifted puller (in the ring at x=-1 or 3, y=0) has
  no seamless power cell: its z neighbours are S, below is the floor, above is the row-1 ring cell
  (quasi-power needs a source next to (x,1,0): S, the door, row 1's piston home or row 2's ring).
- 20:41Z `--no-ring-fire` (a piston in S cannot fire): NO seamless bottom-row mechanism with <= 4
  added sticky/normal pistons or stone blocks (x -5..5, z -2..2, 263k combinations). Hand
  argument: with no firing in the ring, a bottom-row block moves only by pulls from x<=-2 / x>=4
  (faces reach x=0 / 2) or push lines starting in the ring, so the middle block can only reach
  x=0 by a push from the right through a spacer at x=3, which the right side block needs as its
  exit. So the bottom row's middle block must go UP (elevator) for a seamless ground door, or a
  power trick must reach a bottom-ring piston (its only non-S neighbours: the floor, the row-1
  ring cell, its home cell).
- 20:44Z mech-wire on ground20.ideal (uniform gap 5, 128-tick waves, motion triggers): 0 lever
  spots reach every receiver. A self-timed sequencer needs its own search (couplings: observer
  watching a cell piston X changes -> piston Y), not route2.

## FIRST GROUND DOOR: g99 (2026-10-18 21:32Z)
`g99.txt` (= BEST.txt, archive/ground-g99-99b.txt): **99 blocks, volume 462 (11x6x7)**, open/close
54/54, slime-free, rotation-safe, `door verify --extra-offsets 28 --ground`: WORKS at 32 positions,
on the ground (nothing below the door at any tick, works on a stone floor at 32 positions). Not
seamless (the ground board does not need it: vjeux via the coordinator, 20:48Z).
* Mechanism: shift2 (21 blocks). Control: 76 observers + lever + its block, NOTHING else: a tree
  of observers rooted at an observer watching the lever (one 2 gt pulse on BOTH edges), one
  driver observer per (piston group, step) firing into its piston (stacked rows: the driver
  above the top piston facing up powers it and quasi-powers the one below; P0-shifted by
  quasi-connectivity through (3,1,0) from an observer at (3,1,-1); the QC'd piston gets its
  update from the observer's output updating (3,1,0)'s neighbours).
* Timing (2 gt units after the first step): rows 1-2 (left) 0,2,5,7,9,12 + far puller (right)
  14; row 0 (right) 0,2,5,8,11,14 + far puller (left) 14. Observers on a cubic grid have a
  fixed parity per time (cell parity = root parity + time), so gaps had to be chosen per side
  to match the drivers' cell parities (`wire/shift2-obs-lr.ideal` validates them with ideal
  marker blocks: works at 16 positions x 3 cycles).
* Tool: `mctas door obs-chain <spec>` (obschain.rs): driver options per group + times; DFS over
  driver choices with exact-length observer paths (BFS-distance pruned), branch and bound on
  the observer count. Spec: `wire/shift2.chain` (`--shift 11`). Chain observers may output
  into the ground check's floor (harmless: only the observer stands on that floor block).
* `door verify --ground` (new): swept shape below + ground check at the judge's positions.
- 21:45Z **v225** (`v225.txt` = BEST, archive/ground-v225-108b.txt): **volume 225 (9x5x5)**, 108
  blocks, 54/54, rotation-safe, ground check at 32 positions. Same mechanism, obs-chain inside
  box x -3..5, y 0..4, z -2..2, lever (1,3,-1) on the side of (1,3,0), root (1,3,-2).
  Boxes that fail: y <= 3 (PS4's only driver (-1,3,0) faces up: its watched cell is y = 4),
  z -1..1 (P0-home's drivers (4,0,+-1) watch z = +-2), z -2..1 (R1H and R6H both need
  (4,0,-1)). Next: conductor options (an observer powering a solid block next to the piston).
