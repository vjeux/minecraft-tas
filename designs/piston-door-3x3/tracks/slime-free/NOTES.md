# slime-free track: the "Solid, no slime or honey" board

Goal: beat logic-min's slime-free 53 (`archive/logic-min-slime-free-53b.txt`): no slime_block or
honey_block anywhere.

| file | blocks | note |
|---|---|---|
| base53.txt | 53 | logic-min's slime-free 53, the starting point |
| tl-up-b.txt | 52 | TL goes UP (puller at (0,4,0)); the left column keeps only L1/BL pistons, fired by an observer watching TL's cell |
| sf51.txt = BEST.txt | **51** | `door grow` re-laid the B chain's link: O_b -> cross dust standing ON O_b -> block (3,6,0) -> R_B (3 blocks -> 2) |

## Tools (this track)
- `door grow <design> --lo x,y,z --hi x,y,z --states obs,block,dust,... --k 2`: add 1-2 blocks
  in the free cells of a box to a (cut) design, screen every combination (~1500/s on 8
  workers), judge the survivors. Found the 51 in 48 s (64k pairs).
- `door regrow <design> --size 2|3`: cut every face-connected group of control blocks and
  re-grow it with one block fewer (grow on every group).
- Negative results: the TL puller cannot be fed with 2 blocks (obs/block/dust, box
  x -2..2, y 3..6, z -2..3: 300k pairs, none); the right column's dust+floor cannot become 1 block.

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
