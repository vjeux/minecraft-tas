# Track vmid: 7x8 flat FULL-seamless doors whose middle-row side blocks leave VERTICALLY

Brief (coordinator, 2026-10-19 01:17Z): design by hand, outside the catalog's bounds, mechanisms where
ML (0,1) / MR (2,1) leave up or down instead of sideways, so row y=1 is not blocked end to end
(x -2..4) and the top and bottom of a 7x8 box can be wired together. Result: five mechanisms that
open and close with ideal inputs at 32 positions, no wired door (wiring blockers below).

## Mechanisms validated with ideal inputs (`mech/`, all `door ideal <plan> --extra-offsets 28 --cycles 3`: WORKS)

| id | box | pistons | how the 9 blocks leave | open / close (ideal, 6 gt per input) |
|---|---|---|---|---|
| vm1 | x -2..4, y -2..5 | 11 (MR rides TR's input by QC: 10 inputs) | TL left (-2,2); **ML up 2 by a column-0 elevator** (E1 (0,4) lowered into (0,3) by E2 (0,5), pulls ML to (0,2), lifted back, pulls ML to (0,3)); C up 2 by the column-1 elevator; TM up then right ((3,3)); BL left (-2,0); BM, BR down; TR + MR right ((4,2)'s input QC-fires (4,1)). Plan `vm1b.ideal` | 92 / 92 |
| vc1 | x -2..4, y -3..4 | 12 (11 inputs) | **ML up 2 by a CARRIED puller**: P (-1,3) (sticky, facing down, resting in the corner) is pushed into (0,3) by H (-2,3) together with TL (lifted to (0,3) by U (0,4) first, it rides to (1,3)), pulls ML to (0,2), is pulled back by H, U pulls ML to (0,3); TM up (1,4) then right ((3,3)); C DOWN 2 by the column-1 elevator below; BM down then right ((3,-1)); BL down; TR + MR + BR right. Close: (3,3) pushes TM and TL back left together | 92 / 92 |
| vb1 | x -2..4, y -2..5 | 12 | **ML DOWN by a carried puller**: P' (-1,-1) (sticky, facing up) is pushed into (0,-1) by H' (-2,-1) together with the pre-lowered bottom row (BL, BM, BR ride right by one), pulls ML to (0,0), is pulled back, U' (0,-2) pulls ML to (0,-1); C up 2 by the column-1 elevator; TL up; TM up then right; TR + MR right. Close: RP (4,-1) pushes the bottom row back | 86 / 92 |
| vb2 | x -2..4, y -2..5 | 12 | vb1 with BR pulled right by (4,0) (BM's puller had no free neighbor) and the close pusher at (3,-1) | 86 / 92 |
| vb3 | x -2..4, y -2..5 | 12 | vb2's bottom, right side re-planned so every region links: TR UP ((2,4)); MR right ((4,1)) with BR right ((4,0), right below MR's puller: one feed at (4,2) fires both); TM up then LEFT ((-1,3) corner puller); TL left ((-2,2)); the center's elevator fed from (0,4)/(0,5) and (2,5) | 86 / 92 |

`vm.tsv` holds them in families' CATALOG format (`door connect`, `door topology`).

## The carried puller (new primitive, mechanics.md section 8)
A middle-row side block needs a piston in the ring cell above or below its column for its first
vertical move (pos+2 = (0,1) means a base at (0,3) or (0,-1)). An elevator brings it there from
(0,4) with a lifter at (0,5): 3 rows. A sticky piston resting in the CORNER cell and pushed one cell
sideways by a piston two cells out ((-2,3) or (-2,-1)) does it in 2 rows, and the same pusher pulls
it back (pull-type once its front is empty). Whatever stands in the ring row on its way rides along
(vc1: TL; vb*: the lowered bottom row), which is how the parked blocks make room.
Drivability decides the side: facing UP (vb*), its quasi cell is the opening cell (0,0), so a plain
stone in the ring cell (-1,0) powered strongly from (-2,0) fires it (allowed for FULL: quartz-like
blocks resting in the ring are not circuitry). Facing DOWN (vc1), its quasi cell (0,4) holds the
finishing puller and its other neighbors are swept, so it cannot be fired in-plane without that
puller also firing (and pushing it). vc1 is therefore not wirable flat.

## Wiring verdicts
`door connect designs/piston-door-3x3/tracks/vmid/vm.tsv --boxes 7x8 --verbose` (necessary condition:
regions, feeds, links, every event order):
* vm1: NOT wirable. 5 regions [5, 1, 6, 7, 5]: the two elevators side by side split the top (the
  left top feeds E1/E2/TL's puller, the right top feeds the center elevator, TM, TR); with the lever
  in the top-right region it opens but never closes.
* vb1: BM's puller (1,-2) has no feed (neighbors U' and BR's puller).
* vb2: 4 regions [2, 10, 4, 7], vb3: 5 regions [2, 2, 7, 4, 8], vc1: 4 regions [10, 2, 7, 4]: every
  region wakes from most lever regions; the order search caps (300k to 5M states, 12 pistons).
  By hand (vb2/vb3): U' (BL's down-pull, fed only from the 2-cell region (-2,-2), (-1,-2)) and BM's
  puller (fed only from the bottom-right region) must both act BEFORE H's row push, but the
  bottom-left region can only see H' or P' move (the row push itself) and, in vb2, the bottom-right
  only sees the row push too. vb2: not wirable. vb3: not wirable from any lever region. Lever in
  K1/K2/K4: the bottom-left region (U') sleeps until H' moves, and H' must not move before U' acted.
  Lever in the bottom-left region: the middle-left region (H', the ring-stone feed of P') wakes only
  from H' (fed from the lever's region: but H' must wait for BM's puller) or from TL's puller (fed
  from K1 or K2, which need waking first): a cycle. A dummy H' stroke (P' pushed under BL and pulled
  back) would wake the bottom-left on open, but on close the stroke shifts the parked bottom row
  (P', ML, BL, BM and the retracted RP all move). Links counted: a region watching a piston or block
  that moves in a cell next to it; BUD links (a quasi-armed piston fired by a neighbor's motion) give
  nothing here (U' and BM's puller have no quasi source but door blocks that nothing powers).

Why freeing row 1 is not enough in 7x8 (what every layout above ran into):
1. A top-bottom crossing on the left needs x = -2 free at y = -1..3 (x = -1 is ring at y = 0..2),
   so TL, ML and BL must all leave vertically; TL then has to park in row 3 or a corner without a
   piston at (-2,3), which in practice means riding a push along row 3 (vc1) or going up.
2. The center needs 2 moves through TM's or BM's cell: an elevator, 3 rows on that side, and its two
   pistons split that side of the box into left and right regions (the carried puller cannot serve
   C: (1,3) and (1,-1) touch no corner).
3. ML's own vertical exit costs 3 rows (elevator) or 2 rows plus a carrier at (-2,-1) / (-2,3),
   which cuts the free left column from the bottom (top) row.
4. The bottom pullers (2 rows below) split the bottom: with 8 rows the free regions are 4-5 small
   pieces whose only links are piston motions, and the order constraints above break the chain.

## cosearch (explore/cosearch's `--assist`, built in a side worktree)
* vc1 / vb2 without `--lock-pistons` (16 threads, 300-900 s): 75/79 with 2 plan inputs still
  assisted ((4,2), (4,0)); the annealer drifts back to standard side pullers (row-1 wall).
* vb2 / vb3 with `--lock-pistons` (16 threads each, 2700 s): best 75/79 (vb2, 2 assisted) and 73/79
  (vb3, 1 assisted: TR's puller (2,4)); no perfect screen. The partials add pistons and put circuitry
  in ring cells: not near a door.

## Tools
* `door fullmech --vmid ml|mr|any|both` (6c2d7b8): families' vertical-middle candidates on the exact
  model (families' own `--vmid any` run was macro-only, P <= 10, 13k unresolved).

## Next ideas
* A center exit that needs no elevator (or an elevator whose lifter is outside the split, e.g. the
  lifter carried in sideways) is the real key for 7x8: the vmid exits only remove the row-1 wall.
* Let the bottom-left region grow: a 7x8 placement with 3 rows below (y -3..4) gives row -3 under the
  carrier, but then the center must leave downward too and the bottom ring row runs out of cells.
* Make `door connect` decide 12-piston mechanisms (partial-order reduction over independent pullers);
  it caps on every vmid mechanism.

## Log
* 01:33Z vm1 WORKS with ideal inputs at 32 positions (11 pistons: families' P <= 10 run missed it).
* 01:47Z vc1 (top carried puller), 01:56Z vb1, 02:02Z vb2, 02:12Z vb3 (bottom carried puller): all
  WORK at 32 x 3. connect: vm1 not wirable, the others cap; hand analysis above.
* 02:32Z `door fullmech --vmid`; docs: mechanics.md section 8 (carried puller, ring-stone relay),
  history.md Below 72 (vmid verdicts).
