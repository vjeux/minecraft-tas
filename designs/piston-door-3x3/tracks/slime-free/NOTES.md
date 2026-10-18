# slime-free track: the "Solid, no slime or honey" board

Goal: beat logic-min's slime-free 53 (`archive/logic-min-slime-free-53b.txt`): no slime_block or
honey_block anywhere.

| file | blocks | note |
|---|---|---|
| base53.txt | 53 | logic-min's slime-free 53, the starting point |
| tl-up-b.txt | 52 | TL goes UP (puller at (0,4,0)); the left column keeps only L1/BL pistons, fired by an observer watching TL's cell |
| sf51.txt | 51 | `door grow` re-laid the B chain's link: O_b -> cross dust standing ON O_b -> block (3,6,0) -> R_B (3 blocks -> 2) |
| combo.txt = BEST.txt | **48** | logic-min's slime-free 50 (right column = O_R + dust + floor; R_P reads O_a's stone; torch QC-powers P_T) + this track's TL-up left side and grown B-chain link (patches tl-up, tl-up-b, bchain-grow on lm50.txt) |

## Tools (this track)
- `door grow <design> --lo x,y,z --hi x,y,z --states obs,block,dust,... --k 2`: add 1-2 blocks
  in the free cells of a box to a (cut) design, screen every combination (~1500/s on 8
  workers), judge the survivors. Found the 51 in 48 s (64k pairs).
- `door regrow <design> --size 2|3`: cut every face-connected group of control blocks and
  re-grow it with one block fewer (grow on every group).
- Negative results: the TL puller cannot be fed with 2 blocks (obs/block/dust, box
  x -2..2, y 3..6, z -2..3: 300k pairs, none); the right column's dust+floor cannot become 1
  block; on the 51, no group of 2 control blocks re-grows as 1 (8.6k candidates) and no
  connected triple as 2 (obs/block/dust/rep4, margin 1: 1.7M candidates, 16 min).
- Failed: X dust -> a block powered by observers (A gets power by QC through B's head when
  down, but nothing UPDATES A there: the close pulse leaves C pulled back). Lever at (4,4,0)
  (would shorten the A/B chain by 3): the lever QC-powers the top-right piston (4,2,0) through
  (4,3,0), which then never pulls.

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

## 48 (BEST.txt): block budget, 19 mechanism + 29 control
Mechanism: 9 door stones; TL puller (0,4,0), A (1,4,0), B (1,5,0), P_T (1,3,2) (extended, head
(1,3,1)); left pair (-2,0..1,0); right column (4,0..2,0); P_BM (1,-2,0) = 10 pistons.
Control: lever (5,4,0) 1; right column O_R (5,3,0) + cross dust (5,2,0) + floor (5,1,0) 3;
left = TL chain 3 + observer (-1,2,0) + block (-2,2,0) 2; P_BM = O_m (2,-1,0) + block 2;
A feed = O_a (4,4,0) + stone (3,4,0) + X (2,4,0) + floor (2,3,0) 4; B chain = O_b (4,5,0) +
dust on O_b (4,6,0) + block (3,6,0) + R_B (2,6,0) + O (2,5,0) 5; P_T = R1 (5,4,1) + floor +
block (5,4,2) + OR dust (4,4,2) + floor + R_P (3,4,1) + floor + block (3,4,2) + torch (2,4,2) 9.
Verified at 32 and at 104 positions. Open 24 gt, close 50 gt.

## Searches that found nothing on the 48
- `door minimize --moves all --plateau` (incl. main's swaps: 517k "two out, one in").
- regrow size 2 (7k), size 3 with obs/block/dust/rep4 (1M, 4 min), size 3 restricted to the
  P_T cluster with every state (repeaters 1-4, torches, glass: 622k).
- grow 2 blocks for the TL puller with obs/block/dust/rep/rep4/torch/glass (box x -1..2,
  y 3..6, z -1..3: 206k pairs).

## Ideas examined and why they do not pay (yet)
- TR up as well (mirror of TL): the TR puller would sit on X's cell (2,4,0); A then needs a new
  feed (dust at (1,4,-1) on (1,3,-1) + a path for both O_a's and O's pulses) that costs more
  than the 2 blocks it saves. A redstone-block shuttle for A (fresh-c's trick) has the same
  problem: every cell next to A is taken by X, the TL puller or B, and a redstone block cannot
  turn a torch off through a block (it only powers components and dust).
- Pulse-driven P_T (no torch/OR): its open pull must come at 5..10 gt and its close push at
  exactly B-off (20); one pulse source cannot do both. observer-seq's close (B locked on close,
  a late A pushes T and C home) removes that, but the lock (torch + repeater) and the late-A
  path cost about what the 9-block level cluster costs.
- "Center down" (mirror dance under the door, top row of 3 pullers on one wall torch): B' faces
  up, so every cell that powers A' is also a quasi-power cell of B'; and the P_M timing needs a
  level signal from the lever, which then sits far from the top torch.

## Next ideas
- fresh-c's close in a slime-free door, VALIDATED with ideal inputs (`door ideal asteady.sched`,
  mechanism ideal-asteady.txt = the 48 without its control, A extended at rest): A held by a
  steady source (off on open, on again on close after P_T brought T back above C), P_T steady,
  X pulses p0 (close: A pushes C down one) + the two B-edge pulses, B window unchanged (on close
  A is extended, so B cannot push it and the pulses are swallowed). Windows (one at a time):
  a_off 1..5, pt_off 7..12 (open); pt_on 5..9, a_on 5..12 (close); pt_on = a_on works (6..9,
  16 positions): A waits for T to land. So P_T = NOT(lever through a d3 repeater) (no OR, no
  R_P: -2) and A = NOT(lever OR that repeater); the TL puller could then be steady on A's torch
  (one dust instead of the 3-observer chain). The catch is A's fast-off/slow-on source: a torch
  attachment fed by both the lever and the repeater does not fit next to A (every cell around
  A is taken, and a lever cannot be both adjacent to the attachment's dust and behind the
  repeater); a redstone-block shuttle (fresh-c) gives that asymmetry for free but needs a free
  cell next to A for the block at rest and another for it when open.
- A cheaper TL feed: the TL puller needs 1 (or 3) pulses; with the P_T torch moved to (0,4,2)
  one observer at (0,4,1) would do (-2), if the OR net can reach a torch attachment on the west.
