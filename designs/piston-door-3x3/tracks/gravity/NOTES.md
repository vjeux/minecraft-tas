# Track gravity: unconventional materials

Direction: (1) a gravity door (all 9 door blocks sand, `category gravity`), as small as possible;
(2) material tricks (glazed terracotta, honey/slime, special piston behavior) for `category solid`.

## Best: g10, 45 blocks, category gravity, empty walkway (`BEST.txt` = `g10.txt`)

g7 (= g6 with the lever off the walkway) + three steps, each a `door patch` or minimizer result:
* **g8 (48): S2 is pulse-driven.** At rest S2 is RETRACTED at (1,-4,0) with an empty cell above it
  and R3 floating at (1,-2,0) (pistons do not fall). RS2 + its obsidian floor are replaced by one
  observer OS2 at (0,-4,-1) facing north: it watches the A dust (0,-4,-2) and fires into the block
  (0,-4,0) next to S2. Open: R3 retracts (+2), the A dust changes (+2), OS2 pulses (+4..+6): S2
  extends into the gap and retracts, pulling R3 down one cell (+6); S1 then pulls the chain (+10).
  Close: S2 is lifted by S1; OS2's pulse makes S2 push R3 + sand up and SPIT (the pulse ends while
  R3 is still moving, so S2 retracts alone); R3 lands at the top and extends (QC from T_A2).
  OS2 is adjacent to the slime chain, so it RIDES the chain (down when open, up when closed).
* **g9 (46): S1 loses its fast path** (minimizer). Because OS2 rides the chain, in the open state
  it no longer watches the A dust; on close it fires only when S1 brings it back up, i.e. exactly
  when S2 must push R3. So the close sequences itself and S1 can simply be A + 8 gt (RS1).
* **g10 (45): RS1 takes the floor of an existing A dust as its input** ((-1,-6,-2) under dust
  (-1,-5,-2)) and points into a new block (1,-6,-2) beside S1, which drops the dust/floor pair that
  fed it. Verified at 124 positions. Timing: open 5 ticks, close 19 ticks (g8, 48 blocks, keeps
  close 11 for the speed boards).

Control of g10 (21 blocks): lever + lever block; 2 cross dusts + floors + target blocks (P_L, P_R
= lever); T_A2 (R3, QC, top position only); T_A + 3 dusts + floors (A net); RS1 + floor + block
(S1 = A + 8 gt); OS2 + its block (S2 pulses). Mechanism (24): 9 sand, 8 backbone slime, P_L, P_R,
R3, S2, S1, 2 chain slime.

## Earlier best: g4/g6/g7 (51 / 49 / 49)
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

## g6: 49 blocks, category gravity (current BEST.txt)
g4 with T_A (the torch that starts the A net) hung on the floor of the left cross dust instead of
on the lever block: a dust's floor is a strongly powered block, and this one is one level lower,
so the torch's first dust step and its floor disappear (-2). Verified at 124 positions.

## What did not pan out (materials, solid category)
* Glazed terracotta door blocks: they can be pushed but never pulled or dragged, so every door
  block would need a pusher on both sides of its travel; the side columns can only be pushed
  outward from inside the door. No useful solid design came out of it. GT (or obsidian) is
  still the right floor next to a moving slime structure (g1 used obsidian floors).
* Honey instead of slime: same block counts here (honey drags sand like slime does, and honey
  is not a redstone conductor while slime is).
* A slime platform lifting two columns at once: slime does not fall when the piston under it
  retracts, and the 12-block push limit is hit immediately.
* Moving power sources (redstone blocks riding the chain) cannot reach S2: no cell is next to
  both a chain slime and S2.
* Chain top/bottom as a powered conductor (slime conducts): a repeater into the chain bottom
  powers S2 and quasi-powers S1, but none of the repeater positions that point into it have a
  cheap input in the current layout.

## sf4: 62 blocks, slime-free gravity (`sf4.txt`; `door gen sf4-base` + the A-net patch)
sf3's mechanism with cheaper side control: each side's two pullers hang off ONE torch beside the
middle puller (T_L at (-2,1,-1) facing south: it powers the middle puller, and the block above it
(-2,2,-1), which it strongly powers, powers the top puller). The torch's block is fed by an L dust
that runs ON THE WALKWAY FLOOR from the lever ((0,0,-2), then a step up to (-1,1,-2) pointing into
the torch block): the walkway check is "no collision", so dust/torches/levers may sit there.
VL/VR are QC-powered by torches T_AL/T_AR hanging on those walkway dusts' floors (they sit next
to VL's/VR's head cells). The A net for the tower starts under T_AL.

## Router
`door route` now widens its search box to the design bounds + a margin (the old fixed box stopped
at y=-6, the gravity towers go to y=-8).

## Session 2 (07:00-08:00Z): what else was tried
* g8 -> g10 (above). The minimizer (`--moves all --plateau --plateau-pairs`, `--walk 100`) finds
  no move on g10: 45 is a strict local minimum for it.
* Walkway-free variants: g7 (49) and sf3w (64) move the lever from the walkway floor onto the
  lever block's north face (same count); g8-g10 keep it there.
* sf5 (slime-free with a pulse-driven S2 fired by an observer watching VL + one watching RS1):
  the open works, but without S1's fast path the close fires S2's pulse while R3 still sits in
  S2's top cell (next to S2's pulse block) -> R3 extends low. With a fast path it is no smaller than
  sf4 (62), so dropped. A chain-riding observer (what makes g9/g10 self-sequence) needs slime.
* Ideas that cost more than they save (counted, not built): lever next to both side targets (the
  only cell, (1,-1,-1), must stay inert or R3 is QC-powered when open); glass under a walkway lever
  (frees the side power, but T_A2 then has no L block to hang on); S1 above the chain facing down
  (needs an AND: early off on close, late on on open); redstone blocks riding the side structures
  (R3 off at +1 instead of +2 is not enough to drop S2's delay); the tower chain at x=0.
