# Track cosearch2: finish the cosearch partials (f131X 7x8 = 56, w63 7x9 = 63)

Brief (coordinator for vjeux, 2026-10-19 02:28Z): continue track cosearch (redsim joint mechanism + wiring
search, `door cosearch search --assist`): a 1-wide, slime-free, FULL seamless + hallway door below 72b;
targets 7x9 = 63 and 7x8 = 56. 02:35Z: also finish wire131's f131X (8x8-131 with every puller
level-held, 7x8 x -2..4 y -2..5, bottom half wired for real).

## 1. The old 77/79 partials are broken doors (read before continuing any of them)

`door cosearch trace --assist <plan> --assist-cells <cells> --grid x0,y0:x1,y1` (new) replays a
partial with its still-assisted inputs and draws the plane tick by tick. On cosearch's
`partials/w63-2as.txt` ("5 of 7 inputs wired, 77/79"): the CENTER NEVER OPENS (it is still at
(1,1) when the open phase ends, both cycles); the elevator (A x3, B x2, P_T) is not wired at all
(A fires once at t2, B and P_T never); 77/79 = 79 minus the center's open point in 2 cycles. The
search traded a door cell (1 point per cycle) for an assist release (`--assist-weight` 4): with
weight 4 every "released at 77/79" count is meaningless. Also: its rest state needs a redstone
TORCH CLOCK that runs ~30 ticks during settling (torch (3,4) toggling the elevator) until it burns
out (8 turn-offs in 60 gt); the real game agrees (door trace: settled in 34 ticks, torch off), but
it is a fragile trick.

Fix: `--assist-weight 0.45` (< 1 screen point: a release never pays for a broken door cell), and
the search now keeps the best FULL-POINTS candidate apart (`partial-full-live.txt`, fewest inputs
still assisted): the only meaningful partial.

## 2. f131X (7x8 = 56)

Seed `seeds/f131X-lever.txt` = wire131's f131X-brbl + a lever outside the box; plan
`seeds/p131X-brbl.ideal` (6 assisted inputs: D1 (1,4) a/c shots, D1 lowered (1,3) b shot, X
(-2,3), DE window (0,5), D2+ hold (3,4), R pair (4,2)).

* Run a2 (24 threads, weight 0.45, focus = rows 3..5 + (4,2), (3,2)): full-points partial with 4
  inputs left after 3 min (`runs/f131x-a2-full4.txt`): D2+ is held by a wall torch (3,5) on the
  lever's block (4,5) (QC through (2,5), the torch updates D2+ two cells away); D1's b shot comes
  from a redstone LAMP at (-1,5) lit by DE's input cell (0,5), watched by an observer (-1,4) (facing
  up) watched by an observer (0,4) (facing west, emitting east: D1 at (1,4) directly, D1 lowered at
  (1,3) by quasi-connectivity through (1,4)). The lamp going dark 4 gt after DE's window also gives
  the c shot (t34).
