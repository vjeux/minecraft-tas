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
  relaxed speed).
