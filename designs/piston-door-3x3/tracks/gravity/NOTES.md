# Track gravity: unconventional materials

Direction: (1) a gravity door (all 9 door blocks sand, `category gravity`), as small as possible;
(2) material tricks (glazed terracotta, honey/slime, special piston behavior) for `category solid`.

## Best: g4, 51 blocks, category gravity (`BEST.txt`, `mctas door gen g4`)

Built in `crates/piston-door/src/designs_gravity.rs::g4`. Mechanism ("drop tower"):

```
 side view x=1 (z: north <- -> south)          front view z=0
   y=2      T                                    s T T T s      s = slime backbone
   y=1      C                                    s C C C s      L/R columns are sand too
   y=0      M                                    s M M M s
   y=-1  .  h(R3)   T_A2 LB                      s P h P s      P = P_L/P_R (sticky, pushers)
   y=-2     R3          lever on LB              R3 below h
   y=-3  c  h(S2)                                c = slime chain (x=1, z=-1)
   y=-4  c  S2
   y=-5  h(S1)
   y=-6  S1
```

* **Sides**: the side columns ride slime backbones (x=-1 and x=3, y=-1..2) and are PUSHED open by
  sticky pistons that are themselves the doorway floor under the side columns (P_L at (0,-1,0)
  facing west, P_R at (2,-1,0) facing east). Once pushed, the side sand rests on the piston
  heads. They are powered by the lever (on when open): no torches, one cross-shaped dust per side
  from the lever block LB into the block next to each piston.
* **Middle column**: the 3 sand blocks stand on a 3-piston tower in a shaft under the door:
  R3 (regular, top), S2 (sticky), S1 (sticky, bottom, under a 2-slime chain at z=-1 that touches
  S2 and R3). Opening collapses it top-down: R3 retracts, S2 pulls R3 down, S1 pulls the chain,
  which drags S2 and R3 down; the sand falls 3 cells into the shaft (it lands on R3).
  Closing extends it bottom-up: S1 lifts chain + S2 + R3 + sand, S2 lifts R3 + sand, R3 lifts
  the sand into the door.
* **Control** (A = NOT lever, two torches on LB):
  * R3 is powered by quasi-connectivity from T_A2 at (1,-1,-1) (next to R3's head cell). QC only
    reaches R3 at its top position, so R3 waits for S2 by itself (no delay line).
  * S2 = A + 4 gt (repeater RS2 into a block next to S2's top position; it must be off whenever
    R3 sits in that cell, i.e. in the open state, which A guarantees).
  * S1 = A OR A + 8 gt: first on, last off (fast: the floor of an A dust touches S1; slow: RS1).
* Timing: R3 retracts at +2, S2 pulls at +6, S1 pulls at +10, the sand lands ~+16.
  The collapse must be fast (3-4 gt apart) or the falling sand lands on a moving piston and
  breaks into an item.

History: g1 69 (sides pulled by NOT-lever pistons like v2: 14 blocks of side lines) -> g1-min 67
-> g3 60 (sides pushed by lever-powered floor pistons) -> g4 51 (chain + S1 on the north side:
S1's fast input is the floor of an A dust).

## Simulator fix (merged to main as 393a327)
`Sandbox.idle()` ignored falling blocks: the judge stopped while sand was mid-air and failed on
the `falling_block` entities. It now also waits for falling blocks to land (stricter: an item
from a broken falling block still fails the door).

## Mechanics learned (sand)
* `FallingBlock`: onPlace / any shape update schedules a tick 2 gt later; the tick makes it fall
  if the block below is air/fire/liquid/replaceable. Sand moved by a piston is placed at the end
  of the move and gets that tick: it falls 2 gt after landing unless something is under it.
  A `moving_piston` below counts as support.
* Slime stickiness does not hold sand up; only the block below does.
* Sand cannot stay above the doorway in the middle column when open (the support would be a
  door cell), so a gravity middle column has to go DOWN 3 and come back up 3.
* An isolated dust is a dot (powers nothing sideways); write it as a cross in the design
  (`redstone_wire[north=side,east=side,south=side,west=side]`) and the shape update keeps it.
* Loop trap: a repeater pointing into a block that is the floor of the dust net feeding it
  latches the whole net on (tried it for S1's OR: g5, broken).

## Failed / rejected
* v2 with sand door blocks: the center falls as soon as the bottom-middle drops (and after A's
  first pull it hangs over air). The UP dance cannot work with sand.
* Middle column lifted by pistons from above: sand falls back 2 gt later.
* Tower collapse with 6/12 gt spacing: the falling sand hits moving pistons and breaks.

## Slime-free gravity: sf3, 64 blocks (`sf3.txt`; `mctas door gen sf1` + the two patches below)

No slime or honey anywhere (`door verify` prints slime-free). Mechanism:
* **Sides**: the bottom sand of each side column drops 1 cell onto a vertical piston (VL at
  (0,-2,0), VR at (2,-2,0), heads are the doorway floor; powered for free by the floor of the A
  dust next to T_A). The middle and top sands are pulled out by two sticky pullers per side
  ((-2,1..2,0) facing east, (4,1..2,0) facing west) and land on a block at (-1,0,0)/(3,0,0).
  Each pair of pullers is powered by ONE dust standing on a block beside the middle puller: the
  block powers it, the dust points into the top one (the dust climbs from the A net: 4 steps).
* **Middle**: the drop tower without the chain. S1 can only pull S2, so S2 gets a second pulse:
  observer O (1,-5,-1) watches S1's slow repeater RS1 (1,-6,-1); when RS1 turns off (S1 pulls),
  O pulses into B2 (1,-4,-1), the block that also carries S2's normal A+4 signal (RS2). B2 sits
  next to S2's top cell and quasi-powers S2 in its bottom cell, so S2 extends and retracts once
  more and pulls R3 down the last cell. On close the pulse lands while S2 is already on.
* sf1 (68): three pullers per side. sf2 (66): bottom side sands drop onto vertical pistons.
  sf3 (64): S1's fast input is a dust pointing into a block beside S1.
