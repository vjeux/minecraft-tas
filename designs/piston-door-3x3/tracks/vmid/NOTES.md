# Track vmid: 7x8 flat FULL-seamless doors whose middle-row side blocks leave VERTICALLY

Brief (coordinator, 2026-10-19 01:17Z): design by hand, outside the catalog's bounds, mechanisms where
ML (0,1) / MR (2,1) leave up or down instead of sideways, so row y=1 is not blocked end to end
(x -2..4) and the top and bottom of a 7x8 box can be wired together.

## Mechanisms validated with ideal inputs (`mech/`)

| id | box | pistons | how the 9 blocks leave | ideal replay |
|---|---|---|---|---|
| vm1 | 7x8 (x -2..4, y -2..5) | 11 (MR rides TR's input by QC: 10 inputs) | TL left (-2,2); **ML up 2 by a column-0 elevator** (E1 (0,4) lowered into (0,3) by E2 (0,5), pulls ML to (0,2), lifted back, pulls ML to (0,3)); C up 2 by the column-1 elevator; TM up then right ((3,3) puller); BL left (-2,0); BM, BR down; TR + MR right ((4,2) input QC-fires (4,1)) | `door ideal mech/vm1b.ideal --extra-offsets 28 --cycles 3`: WORKS (32 positions x 3 cycles), open 92 / close 92 (1-tick pulses, 6 gt apart) |

vm1 in the macro model (`door families --check mech/vm1.txt`): open 15 + close 9, symmetric train of
15 pulses. Free cells of row y=1: (-2,1) (between TL's and BL's pullers) and (-1,1) (ring, never
swept). It has 11 pistons: families' `--vmid any` run was capped at 10, which is why it missed it.

## Log
* 01:33Z vm1 (two side-by-side elevators on top) WORKS with ideal inputs at 32 positions.
