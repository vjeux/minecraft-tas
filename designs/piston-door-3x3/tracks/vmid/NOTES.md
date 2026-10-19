# Track vmid: 7x8 flat FULL-seamless doors whose middle-row side blocks leave VERTICALLY

Brief (coordinator, 2026-10-19 01:17Z): design by hand, outside the catalog's bounds, mechanisms where
ML (0,1) / MR (2,1) leave up or down instead of sideways, so row y=1 is not blocked end to end
(x -2..4) and the top and bottom of a 7x8 box can be wired together.

## Mechanisms validated with ideal inputs (`mech/`)

| id | box | pistons | how the 9 blocks leave | ideal replay |
|---|---|---|---|---|
| vm1 | 7x8 (x -2..4, y -2..5) | 11 (MR rides TR's input by QC: 10 inputs) | TL left (-2,2); **ML up 2 by a column-0 elevator** (E1 (0,4) lowered into (0,3) by E2 (0,5), pulls ML to (0,2), lifted back, pulls ML to (0,3)); C up 2 by the column-1 elevator; TM up then right ((3,3) puller); BL left (-2,0); BM, BR down; TR + MR right ((4,2) input QC-fires (4,1)) | `door ideal mech/vm1b.ideal --extra-offsets 28 --cycles 3`: WORKS (32 positions x 3 cycles), open 92 / close 92 (1-tick pulses, 6 gt apart) |

| vc1 | 7x8 (x -2..4, y -3..4) | 12 (MR rides TR's input: 11 inputs) | **ML up 2 by a CARRIED puller**: P (-1,3) (sticky, facing down, resting in the corner) is pushed into (0,3) by H (-2,3) together with TL (which rides right to (1,3)), pulls ML to (0,2), is pulled back to the corner by H, then U (0,4) pulls ML to (0,3); TL was lifted to (0,3) by U first; TM up (1,4) then right ((3,3)); C DOWN 2 by the column-1 elevator below ((1,-2) lifted by (1,-3)); BM down then right ((3,-1)); BL down; TR + MR + BR right. Close: TL comes back as the tail of TM's push ((3,3) pushes TM and TL left) | `door ideal mech/vc1.ideal --extra-offsets 28 --cycles 3`: WORKS, open 92 / close 92 |

The carried puller needs no piston at y = 5 for ML: only rows 3-4 above the door, so the center's
elevator fits below (8 rows). `door connect` (vm.tsv): vm1 NOT wirable (5 regions; the two elevators
side by side split the top: lever in the top-right region opens but cannot close); vc1: 4 regions
[10, 2, 7, 4], every region wakes from K0/K1/K2, the order search capped at 300k (re-run at 5M).

| vb1 | 7x8 (x -2..4, y -2..5) | 12 | **ML DOWN by a carried puller**: P' (-1,-1) (sticky, facing up, resting in the corner) is pushed into (0,-1) by H' (-2,-1) together with the bottom row (BL, BM, BR, already pulled down, ride right by one), pulls ML to (0,0), is pulled back, U' (0,-2) pulls ML to (0,-1); C up 2 by the column-1 elevator; TL up; TM up then right; TR + MR right. Close: RP (4,-1) pushes the bottom row back left | WORKS 32 x 3, open 86 / close 92 |
| vb2 | 7x8 (x -2..4, y -2..5) | 12 | vb1 with BR pulled right by (4,0) (BM's puller had no free neighbor) and the close pusher at (3,-1) | WORKS 32 x 3, open 86 / close 92 |

Why the bottom version: a piston facing UP has its quasi cell in FRONT of it, so P' at (0,-1) is
QC-powered through (0,0) by a plain stone in the ring cell (-1,0) that a component at (-2,0) (the
free left column) powers strongly; vc1's P (facing down at (0,3)) has its quasi cell at (0,4) = U and
no free neighbor, so it cannot be driven in-plane without firing U.

vm1 in the macro model (`door families --check mech/vm1.txt`): open 15 + close 9, symmetric train of
15 pulses. Free cells of row y=1: (-2,1) (between TL's and BL's pullers) and (-1,1) (ring, never
swept). It has 11 pistons: families' `--vmid any` run was capped at 10, which is why it missed it.

## Log
* 01:33Z vm1 (two side-by-side elevators on top) WORKS with ideal inputs at 32 positions.
