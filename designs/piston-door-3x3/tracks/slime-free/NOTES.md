# slime-free track: the "Solid, no slime or honey" board

Goal: beat logic-min's slime-free 53 (`archive/logic-min-slime-free-53b.txt`): no slime_block or
honey_block anywhere.

| file | blocks | note |
|---|---|---|
| base53.txt | 53 | logic-min's slime-free 53, the starting point |
| tl-up-b.txt = BEST.txt | **52** | TL goes UP (puller at (0,4,0)); the left column keeps only L1/BL pistons, fired by an observer watching TL's cell |

## 52: TL up, the left pair fired by watching TL's cell
- The top-left door block is pulled UP by a retracted sticky piston at (0,4,0) (next to A),
  fired by a 3-observer chain watching P_T: (0,3,2)E -> (0,4,2)D -> (0,4,1)S (back into the puller).
- The left column keeps 2 pistons (-2,1,0) and (-2,0,0). One observer at (-1,2,0) facing east
  watches TL's cell (0,2,0) (TL leaves/returns once per edge) and strongly powers the block
  (-2,2,0) above the middle piston: the middle is powered directly, the bottom one by
  quasi-connectivity through the middle piston's cell (and updated when the middle one moves).
- Trick worth reusing: **an observer watching a door cell + one block above a vertical pair of
  side pistons = 2 blocks per pair**, fired when the block above the pair leaves.
- Patches: tl-up.patch (53 -> 49, TL puller unpowered) + tl-up-b.patch (+3 observers).
