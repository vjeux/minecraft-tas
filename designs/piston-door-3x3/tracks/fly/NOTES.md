# fly: a 3x3 door smaller than 56 (from 2026-10-20 17:40Z)

Coordinator brief (vjeux: "the door is still #1 priority"): beat 56 (7x8x1, e78k-h5-41b) on the
Solid board: volume < 56, rotation-safe, FULL frame 12/12 when open, solid category. Slime is
allowed on that board (it has its own "no slime" board); every earlier sub-56 hunt (c70b, w86,
macrosat, pockets3, d67) ran under the slime-free model. My angle: slime / observer
"flying" sub-assemblies (one piston moving a whole group, self-timed by observers riding the
moving parts), and box shapes nobody tried.

## Box shapes under 56
A FULL frame needs the 12 frame cells in the door plane, so the door plane is at least 5x5.
* depth 2: only 5x5x2 = 50 (5x6x2 = 60). Never tried (the smallest 3D door on the boards is
  126b, 7x9x2).
* flat: 7x7 49, 6x8 / 8x6 48, 6x9 / 9x6 54, 5x10 / 10x5 50, 5x11 / 11x5 55, 6x7 / 7x6 42.

## 5x5x2 = 50: ruled out (argument, 2026-10-20)
Planes A (z = door, 5x5: opening O, 12 frame cells F, 4 corners) and B (z = door+1: ring Br of
16, center Bc must be air at rest and open).
1. No block ever changes plane inside the box: a piston pushing across z pushes out of the box
   (z = -1 or 2), and a sticky pull across z needs its base at z = -1 or 2.
2. Bc can only be entered by piston HEADS of Br pistons facing in (anything else would need a
   force from outside the box or from a piston resting in Bc); a head is not sticky sideways, so
   nothing in Bc can ever drag a door block. Plane B can only drag plane-A frame / corner blocks.
3. So every door block leaves O by a plane-A force: an edge-middle block (ML) moves west only by
   a row push from the east (a pusher in O or in the opposite frame cell) or by a slime next to it
   in O moving west, and a slime can enter O only from F, which needs a force from outside the
   5x5 or from inside O. The 9 blocks + the 12-block push limit leave no way to empty O and fill
   all 12 frame cells with plane-A pistons standing in the 4 corners and the frame cells (which
   must hold door blocks when open). Dropped.

## Flat 7x7 / 6x8 / 6x9 with slime
State of the art (c70b smaller/NOTES.md): 7x7 mechanisms exist (c77 w1, u0 / u2) but need 17-19
piston actions per edge from free cells that form 5 islands (wiring UNSAT). Slime lets one
piston move a whole column or row (up to 12 blocks) and an observer riding the moved group
sequences the next piston with no wire, so the action count per edge can drop a lot.

### Side structures (redsim, 2026-10-20 17:56Z, /tmp/fly/s0.txt + s0.ideal)
7x7 (x -2..4, y -2..4). Left: slime in the wall cells (-1,0..2), up through the corner (-1,3) to
(-1,4), pushed west by a sticky piston at (0,4) facing west; right mirrored ((3,0..4), sticky at
(2,4) facing east). ONE level action per side: the slime goes to x = -2 / 4 and drags TL, ML, BL
(TR, MR, BR) into the wall cells; the sticky retraction brings everything back. 8 blocks per push
(limit 12). Constraints it leaves: x = -2 / 4 (y 0..4) empty at rest, and every cell next to a
moving slime that is not meant to move must be air at rest: (0,3) and (2,3) (next to the
corners) and (-1,-1), (3,-1). Routing through the bottom corner instead frees (0,3)/(2,3) and
blocks (0,-1)/(2,-1). A slime next to a stone that should end in a frame cell drags it there
(e.g. slime at (0,-1) drags a stone from (1,-1) into (0,-1)).
Open problem: the center column. C is 2 cells from every frame cell; the 7x7 has one row above
the ceiling and one below the floor, so a single pull reaches only 1 cell.

### Why the center stays hard with slime (2026-10-20 18:50Z)
* A slime that drags a block into a frame cell ends beyond it (outer ring) in the drag direction;
  a slime (or piston) inside the 3x3 needs 2 moves to leave it. So C's two moves are: a first
  move onto an edge cell (a push of a line from a ring piston, or a pull by a sticky face in the
  3x3) and a final pull from 2 cells beyond the frame (P_T at (1,4) pulling C from (1,2) into
  (1,3): cheap).
* Every moving slime "poisons" its neighbours: a cell next to a moving slime that must not move
  must be AIR at that moment. A routed wall column therefore blocks (0,3)/(2,3) (top route) or
  (0,-1)/(2,-1) (bottom route) from holding a static frame block. Dragged extras fix that: the
  corner route through (0,3) drags whatever sits at (1,3) into (0,3) (a stone, or TM after P_T
  pulled it up), and the same at (0,-1)/(2,-1) from (1,-1).
* Q trick (new, redsim-checked pieces): a sticky piston Q in the left wall middle (-1,1) facing
  east, stuck to the left structure's slime. Q pushes ML, C, MR east (MR pushes the right
  structure's slime (3,1): that push moves the whole right structure, TR / BR dragged into the
  wall, no pusher needed), Q's sticky retraction pulls ML back: the CENTER IS EMPTY. Then the left
  structure drags Q into the margin (-2,1) (one escape move) and Q pulls ML into (-1,1). w1's
  lemma (the ring piston needs >= 2 escape moves) assumed a separate margin puller; here Q is it.
  What remains: C sits on (2,1) next to MR, still 2 cells from a free frame cell ((2,3), (2,-1)).
* Vertical version (Q in the floor middle pushing BM C TM up, TM dragged into (0,3) by the left
  corner slime, P_T pulling C into (1,3)) works up to Q's escape: Q must leave (1,-1) downward,
  and a down-moving slime next to Q needs a pusher in a cell the wall structures need.

### Searches
* `fullmech --anneal 7x7 --slime --helpers` from scratch, all offsets, 30 min: nothing.
* Seeded with the two frozen side structures (`--anneal-seed /tmp/fly/s0.txt --anneal-freeze
  99 --frame-full`), 20 min, 265k evaluations: nothing.
* New exhaustive `fullmech --complete` (every placement of k pieces from a palette in given
  cells added to a frozen seed, each solved exactly). Calibrated: fm-7x8-000 minus 2 pistons
  re-found (5 of 1313 candidates, 7 s). Seed sW (left top-routed with (0,3), right bottom-routed
  with (2,-1), P_T (1,4), P_B (1,-2), statics (2,3) (0,-1)): k <= 2 over 16 free cells (pistons,
  slime, stone, obsidian) 0 of 24,688; k <= 3: 0 of 1,579,384.
