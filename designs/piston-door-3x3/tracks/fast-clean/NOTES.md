# Track `fast-clean`: the fastest solid doors that respect "nothing in front of or behind"

vjeux's rule (judge on main since 2026-10-18 16:40Z): at rest the 3x3 columns in front of and behind
the opening (x 0..2, y 0..2, every z != 0) hold no block at all, at any distance. Speed boards: open,
close, open + close (worst of 3 cycles at the first position), ties by blocks. k = ticks after the flip.

## Audit (`door verify` on main, 16:52Z)
Every fast solid door breaks the rule in the SAME six cells: the lever net's east branch crosses in
front of the door as a dust staircase at z = -4 (floors (0,0,-4), (1,0,-4), (2,1,-4), dust (0,1,-4),
(1,1,-4), (2,2,-4)); nothing else of the speed family (E/F/A'/Q elevator, R_X/R_Z placers) is in the
columns. Same cells in lamp-sweep a1-107 (10/7), o6-107 (11/7), speed-wire s4-108 (12/7), speed-2
n3-112b (9/7), and tick-door's w9 (its lever line at z = -4, y 1..2).

Why the staircase dips to y 1: dust over a column cell needs its floor outside the column, so a
crossing in front of the door must run at y >= 4 (floors at y 3) or y <= -1 (floors at y -2).

## Tool changes (own commit)
* `door route`, `route2`, the searches (`minimize`, `regrow`, `grow`: search.rs) and `synth` keep the
  whole columns clear (route.rs `keep_clear`: door cells + 32 cells deep each way), not just the 3-deep
  walkway: they no longer propose anything the judge rejects.
* `door route --swept-design <working design>`: swept cells from the design a net was cut out of,
  counting only cells its moving parts visit (static wiring is free), to re-lay a net elsewhere.

## Results (all `door verify --extra-offsets 28`: 32 positions x 3 cycles, rotation-safe)
| file | blocks | open / close | what |
|---|---|---|---|
| n3c-116.txt | 116 | 9 / 7 | speed-2 n3-112b with the staircase cut (9 blocks: the 6 column cells, its west end (-1,2,-4) + floor, its east end (3,3,-4)) and re-laid by `door route --swept-design` over the top at z = -5: (-1,2,-4) up to (-1,3,-5), along y 4 on floors at y 3 (x 0..2), down to (3,3,-5) and (3,3,-4) (13 blocks). Strength at the far ends 3 instead of 5 |
