# Track gravity-2: a cheaper gravity door (sand door blocks)

Board: gravity (the 9 door blocks are sand). Start: synth-g42 (42 blocks, open 5 / close 17).

## Baseline searches on g42
* `door resynth` sizes 2..5 with palette block,dust,dot,rep,obs,torch,lamp,note,bulb: nothing
  (42 -> 42, 35 min). g42's wiring is minimal in that model; the slack is in the layout.

## New layout (t1/t2): side pistons ON TOP, lever between them, free R3 control
* The side pistons sit above the door's side columns: P_L (0,3,0) facing west, P_R (2,3,0)
  facing east, pushing 5-slime backbones (x=-1 / x=3, y=-1..3). The lever hangs from the
  ceiling between them at (1,3,0) (attached to a block at (1,4,0)): it powers BOTH side pistons
  directly. Side control = 2 blocks (g42: lever, lever block, 2 dusts, 2 floors, 2 blocks).
* The side columns' supports ride the backbones: the left structure drags a REDSTONE BLOCK at
  (0,-1,0) (under the left door column at rest, under the moved column when open); the right
  structure drags a stone at (2,-1,0) the same way.
* R3 (the regular piston whose head holds the middle column at rest, (1,-2,0)) is
  quasi-powered by that redstone block through its head cell (1,-1,0): the side push carries
  the redstone block away at t1, so R3 retracts at once (no torch), and on close R3 is powered
  again only when the tower brings it back to the top position (self-waiting).
* Middle column: g42's drop tower (R3, S2 sticky, S1 sticky under a slime chain).

## t2: the 3-slime chain makes the tower symmetric
With a 3-slime chain at z=+1 ((1,-4..-2,1)), S1's pull drags BOTH R3 (still at the top) and S2,
so the order is: open R3 retract, S1 pull, S2 pull (S2 now in its LOWER cell (1,-5,0) pulls R3
from (1,-3,0) to (1,-4,0)); close S2 push+spit (from the lower cell), S1 push+spit, R3 extends.
S2 only ever needs power in its lower cell, and a cell next to (1,-5,0) (e.g. (0,-5,0)) touches
neither S2's upper cell nor R3's lower cell (1,-4,0). So S2 can take a pulse train that is
the same on both edges:
* pulse a1 (early): open: S2 is still up and does not see it; close: S2 pushes R3 up and spits.
* S1 pulse at s: open: pull; close: push+spit (state decides).
* pulse a2 (late): open: S2 (now down) pulls R3 down; close: S2 is up again and does not see it.
Ideal-input sweep `t2.sched` (a1 = 2): works for s = 9, a2 = 13..15 and s = 10, a2 = 14..17
(and later a2 when the sand lands first and falls again). Base mechanism + lever = 31 blocks.
