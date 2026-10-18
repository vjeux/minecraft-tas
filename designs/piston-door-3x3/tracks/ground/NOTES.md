# Track ground: the first door on "Solid, on the ground (nothing below the door)"

Goal (coordinator, 2026-10-18 19:29Z): no block below the door's bottom row, at rest or while it
moves, and it must still work standing on a stone floor (`ground_check`: slime may not drag the
ground, nothing may push or power through it). All other rules apply (nothing in front/behind at
rest, rotation-safe). Start: shapes' `bases/ground20.txt` (20-block mechanism, works with ideal
inputs: `ground20.ideal` per edge, `ground-sym.ideal` one 24-pulse train for both edges).

## Log
- 19:30Z setup: clone, branch explore/ground, merged origin/explore/shapes (its NOTES, sym-bfs
  cell print, shape cache v2). `door ideal ground-sym.ideal --set s=5` WORKS (123/123 ticks).
