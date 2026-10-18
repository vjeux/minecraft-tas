# Track strip: a 1-wide seamless door in a THIN box (12 x 5 = 60, then 11 x 5 = 55)

Brief (vjeux via the coordinator, 2026-10-18 22:13Z, "is 55 possible?"): a flat (1-wide),
slime-free, FULL seamless + hallway-compatible 3x3 door in a box 5 tall: 12 x 5 = 60 first, then
11 x 5 = 55 (record at the start: e89b 72 = 8 x 9). Agent families (explore/families,
tracks/families/ROWS.md) showed that in a box <= 5 tall the door blocks can only slide sideways
in their own rows, each row of 3 solved on its own: <= 4 pistons per row -> rows 12 wide, <= 5 ->
11 wide (198 minimal rows), nothing narrower. Agent flat-seam's `door rows-fire` lists where each
row piston fires from (tracks/flat-seam/strip/ROWS-FIRE.md); flat-seam runs r046 x 3 rows with
ideal inputs (11 wide).

Coordinates: door coordinates, the door at x 0..2, y 0..2, the box rows y = -1 (bottom wiring
row) .. 3 (top wiring row). Ring cells (the hallway's wall next to the tunnel, door plane):
(-1,0..2), (3,0..2), (0..2,-1), (0..2,3). Corner cells (-1,-1), (3,-1), (-1,3), (3,3) are NOT
ring cells (circuitry may stand there).

## Lemma (the corner conflict): rows 0 and 1 cannot be wired with fixed parts in a 5-tall box

What can power a piston in each door row, a 5-tall box with the wiring rows y = -1 and y = 3:
* Quasi-connectivity looks UP only: a piston at (x,y) also reads the signal going INTO (x,y+1).
* Ring cells hold only air, door blocks or plain blocks in the stable states, so no fixed
  component can stand in (0..2,-1), (0..2,3), (-1,0..2), (3,0..2): only a plain stone, which
  powers pistons next to it only while STRONGLY powered by a component next to it.
* Row -1 has no floor under it and row 3 stands on the walking pistons of row 2, so neither can
  hold dust, repeaters or comparators over a walking cell (no sturdy floor).
* Strong power into a block comes from: an observer's output face, a repeater/comparator's
  output (need a floor), dust on top of it, a torch UNDER it, a lever/button on it. A redstone
  block or a torch beside a block does not power the block (they power adjacent components).

Every row mechanism of the catalog (433 rows, `door rows-fire`) has, on one side, BOTH firings:
the inner door cell beside the ring (x = 2: a walker pushes the door line out, then pulls the
edge block back one cell) AND the ring column (x = 3: the same walker pulls the edge block on);
mirrored rows do it at x = 0 and x = -1 (counts: R:11 77, L:11 121 + 24 + 150 + 58, R:11 3 of
the 6-inner rows; nobody avoids it). The 211 rows that fire at x = 1 are dead for rows 0 and 1
outright: (1,-1) and (1,3) can never be powered (their neighbors are ring stones, the door cell
and the outside).

For the right side (the left is the mirror image, corner (-1,y)):
* Row 1, firing at (2,1): direct neighbors (2,0), (1,1), (3,1), (2,2) and the quasi cell's
  neighbors (1,2), (3,2) are door-row cells (door blocks, walkers or air: none can be strongly
  powered by a fixed part); only (2,3) is left, a ring stone, and the only component that can
  strongly power it is an OBSERVER in the corner (3,3) facing east (output west into (2,3)):
  (1,3) is ring, (2,4) is outside, (2,2) is a door cell, and a repeater at (3,3) has no floor.
* Row 1, firing at (3,1): direct neighbors (3,0), (2,1), (4,1), (3,2) are door-row cells (in
  lockstep they hold the other rows' walkers); the quasi cell (3,2)'s neighbors are (2,2), (4,2)
  (door row) and (3,3): the corner must send power DOWN into (3,2): a strongly powered conductor,
  a torch or a redstone block in (3,3). An east-facing observer there only outputs west.
* So (3,3) must be an east-facing observer at the x = 2 firings and a down-emitter at the x = 3
  firing: no single fixed block is both.
* Row 0: the same at the bottom corner (3,-1): (2,0) only from the ring stone (2,-1) (observer in
  (3,-1) facing east), (3,0) only from (3,-1) itself emitting up (its quasi cell (3,1) is in the
  door rows).
* Row 2 escapes: the corner observer's output into (2,3) quasi-powers both (2,2) (quasi cell
  (2,3)) and (3,2) (quasi cell (3,3), next to the powered stone (2,3)).

Consequences (what a full model must allow): rows 0 and 1 need a MOVING part at the corner (a
redstone block or an observer shuttled in row -1 / row 3 by an extra piston, swapped between the
two roles), or a second wiring row on that side (a 6-tall box: 11 x 6 = 66 beats 72), or a row
mechanism without the pattern. A static SAT wiring model of a 5-tall strip hits the same wall.

## Log
- 22:13Z started; merged main + families + flat-seam; `door families --rows` + `door rows-fire`.
- 22:41Z lemma above (sent to the coordinator).
- `door families --rows --forbid-fire=<x,...>` (new, families.rs `FORBID_X`): no piston may act
  from those x columns; enumerating 12-wide rows (arms up to 7, <= 5 pistons) with
  `--forbid-fire=-1,1,3` (each corner only serves the inner cell) ...
  Result: **no row mechanism avoids the corner pattern**, for every single-role assignment of the
  two corners (`--forbid-fire=-1,1,3`: corners serve only the inner cells; `=1,2,-1` and `=0,1,3`:
  one corner inner, the other ring column; `=0,1,2`: no inner firing at all), rows up to 12 wide,
  every arm split (0+9 .. 4+5), <= 6 pistons per row: 0 rows open and close (without a forbid set
  the same runs find families' 198 rows at 11 wide and 222 at 12 wide, so the tool works).
  => **a 5-tall strip door cannot be wired with fixed parts**: row 0 is blocked whatever the other
  rows do (its pistons can only be powered from row -1: QC looks up into row 1), row 1 likewise
  at the top corner unless row 0's door block sits on a torch at (3,-1) at the right moment (which
  row 0's own conflict forbids).
- 6 tall does not save it with fixed parts: the side with a single wiring row keeps the conflict
  (two rows below fix row 0: a torch at (2,-2) strongly powers the ring stone (2,-1), freeing the
  corner (3,-1); two rows above fix row 1: dust at (2,4) on the ring stone (2,3) frees (3,3)); a
  7-tall box (11 x 7 = 77) is worse than the record.
- A second wall for 1-high wiring rows: a run of k adjacent firing columns (r046's x = 3..6 for
  rows 1 and 2) needs k adjacent cells of row 3 that send power DOWN (a strongly powered
  conductor, a wall torch or a redstone block); a conductor needs a horizontal neighbour that
  strongly powers it (an observer facing away, which itself sends nothing down; repeaters have no
  floor), a wall torch hangs on a horizontal neighbour block and is ON by default (every walker
  landing under it fires). Only runs whose both ends touch an outside observer cell can be driven,
  and their torch cells are inverted levels.
