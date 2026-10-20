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
