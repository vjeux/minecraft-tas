# Track `pareto-mid`: the middle of the solid Pareto frontier (open ticks, close ticks, blocks)

Direction (coordinator): the solid frontier at the start was 38 (36/32), 40 (24/9, fc2), 41 (22/9,
fc2f), 43 (16/16, mp8), then nothing until 107 (10/7, lamp-sweep-a1-107). Fill the middle: every
design not dominated on all three counts.

Where a new door lands on the frontier (b = blocks): for 44 <= b <= 106 it is a frontier point iff
open <= 15, or close <= 8, or (open <= 21 and close <= 15). At 43 it must beat mp8 on open or close
and fc2f's 22/9. `mctas door frontier --check <file>` (this track's tool) says it directly.

## Frontier points found (all `door verify --extra-offsets 28`, rotation-safe, category solid)

| file | blocks | open / close | from | what changed |
|---|---|---|---|---|
| `archive/pareto-mid-mp8-leafU-43b.txt` | 43 | 15 / 17 | mp8 (43, 16/16) | U's trigger one tick earlier on the open edge: the bottom-right flag is an oak log, the first observer of U's 3-observer chain becomes persistent oak leaves (2 blocks swapped, same count) |
| `archive/pareto-mid-mp7-repU-46b.txt` | 46 | 14 / 14 | mp7 (45, 16/16) | U fired at t12 on both edges: flag -> observer (3,-2,0) -> block (3,-3,0) -> delay-1 repeater (2,-3,0) on a floor (2,-4,0) -> U (+1 block). Works because mp7's pump does not trip on the close edge (below) |

## The pump trip (why mp8 cannot simply fire U earlier)
mp8's pump repeater R_P points straight into the pump P (0,-2,0). On the close edge the lowered P
sits at (0,-3,0) under the lowered slime row; R_P's t7 pulse strongly powers the slime at (0,-2,0)
and its turn-on updates that cell's neighbours, so the lowered P fires: the row goes up at t7 (U,
glued under it, rides up), P pulls it back at t11, and U is pushed down again, landing at t13 BE,
before the row (block-entity order). So on the close edge U needs a NEW power edge at t14 or later
(an extend queued by its landing finds the slime still moving and is dropped, no retry): close >=
16 with 2 gt pulses, whatever the open edge does. mp7 (fresh-b's previous step, 45 blocks) feeds
the pump through dust on obsidian (R_P -> dust (0,-1,-1) -> obsidian (0,-2,-1) -> P): dust updates
reach 2 cells, the lowered pump is 3 away, so it is never updated and never fires on close (it is
quasi-powered, but stays a silent BUD until the door's own close lifts it back). In that no-trip
world U can fire at the same tick on both edges: U at t12 gives 14 / 14.

Measured with ideal inputs (`mp8-PU.sched`: P and U by redstone blocks, no trip; `mp8-PUr.sched`:
P through its real repeater turned to delay 1, trip modelled), open edge: pump [7,11) -> U must
extend at t12 (2 gt) or t11 (3 gt): open 14; pump [6,9) and U at t10: open 12. Close edge, no trip:
U at t10..t11 -> close 13 (A2's C lands at 13: F's return at t8 sets it); with the trip, close 13
only if the trip ends by t8, 14 if by t9, 15 if by t10, else 16.

`door grow --k 1` (states obs, block, dust, rep1/2, torch, leaves, log; box -4..5 x -6..7 x -2..2):
no single added block gives mp7-repU open <= 13, or close <= 13 with open <= 21; none gives
mp8 / mp8-leafU open <= 14, or (open <= 15, close <= 16).

## Tools (own commit)
* `door frontier [--all] [--check FILE...] [--any-category] [--cache-dir D]`: the frontier from the
  leaderboard's verdict cache (designs not in it are verified), every working design with its ticks
  (`--all`), and each candidate placed against it (ON THE FRONTIER, which frontier doors it pushes
  off, or which doors dominate it).

## Persistent leaves: an asymmetric 1 gt delay (new timing element)
Persistent leaves recompute `distance` in a scheduled tick 1 gt after a neighbor changes
(`LeavesBlock.updateShape` -> `scheduleTick(.., 1)`), and never random-tick (allowed by the judge).
Next to a log carried by a moving structure:
* when the log LEAVES (the cell turns to air in the block-event phase of the move, tick m), the
  leaves change at m+1 (distance 1 -> 7);
* when the log ARRIVES it is a `moving_piston` (distance 7, no change) until it lands at m+2, so the
  leaves change at m+3 (7 -> 1).
So leaves next to the log's REST cell fire an observer chain 2 ticks earlier on the open edge than
on the close edge (leaves next to its OPEN cell: the reverse). Chains of several leaves ripple
(each re-reads the other: 1 -> 3 -> 5 -> 7...), so only one leaves block per log.

mp8 + leaves: the right structure moves at t8. Old chain: stone flag (3,-1,0) -> observers
(3,-2,0), (3,-3,0), (2,-3,0) -> U at t14 on both edges (open 16 / close 16). New: log (3,-1,0) ->
leaves (3,-2,0) (t9 open / t11 close) -> observer (3,-3,0) -> observer (2,-3,0) -> U at t13 open
(open 15) / t15 close (close 17).

## Log
* Background: `door resynth` on a1-107 with `--max-open 12 --max-close 8` (sizes 2..3, full palette
  with lamps/bulbs/notes/targets): 107 -> 107, nothing (the 107's wiring is resynth-minimal even at a
  relaxed speed). `door minimize --moves all --max-open 16 --max-close 8` on a1-107: nothing (no
  removal, pair, swap or move works even at open 16). s4-108 (12/7) is not legal any more (dust and
  stone at z = -4 in the columns in front of the door, rule of 16:40Z), so the speed family can only
  shrink after a re-route.
* `door minimize --objective open --slack 6 --moves all` on mp8: nothing. `--max-open 14
  --max-close 14` on mp7-repU: nothing (46 is a local minimum at 14/14).
