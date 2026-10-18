# Track box70: flat, slime-free, FULL seamless + hallway door in 7 wide x 10 tall (70b)

Brief (coordinator, 2026-10-18 21:34Z): 1-wide (flat), slime-free, FULL seamless + hallway door in a
7x10 box (record 80b: flat-b-v7-full 8x10; flat-seam's e89b is 8x9 = 72b, so only 7x10 or smaller
counts). Lead (boxtall): flat-b-v7's mechanism with the right pullers fired from above and an
observer below them as the top-to-bottom crossing.

## Mechanism m7 (validated with ideal inputs)

[ideal/m7.txt](ideal/m7.txt) + [ideal/m7.ideal](ideal/m7.ideal): `door ideal` WORKS (8 positions x 2
cycles, open 31 / close 31). It is flat-b-v7-full's mechanism mirrored top-bottom, with BL going down
instead of left (= families' 7x8-031): x = -2..4, rows -2..5 (7x8 before wiring).

* center elevator on TOP: P_M (1,4) facing down, lift P2 (1,5) facing down, corner puller P_TL (-1,3)
  facing east. Open: P_M pulls T up to (1,3); P_TL pulls T west to (0,3); P2 pushes P_M down to
  (1,3); P_M pulls C up to (1,2); P2 pulls P_M back to (1,4); P_M pulls C to (1,3).
* left pair (-2,1), (-2,2) facing east (ML, TL into the left ring), right pair (4,1), (4,2) facing
  west (MR, TR into the right ring), bottom pullers (0,-2), (1,-2), (2,-2) facing up (BL, M_B, BR into
  the floor ring). Every piston pulse-driven; one train for both lever edges:
  P_M, P_TL, P2 (level window 8..18, or two pulses), P_M (at (1,3)), P_M, then sides + bottom.
* Open state ring: left (-1,1) ML (-1,2) TL, right (3,1) MR (3,2) TR, floor BL M_B BR, ceiling T (0,3)
  C (1,3); (-1,0), (3,0), (2,3) stay air (FULL-clean: nothing passes through them).
* Order constraint (from the close): the M_B puller (1,-2) must fire after P_M's third pulse (it
  pushes M_B, C and T up together); BL, BR and the side pairs are free.

## Why the wiring is hard in 7 wide (what I found)

* Row 1 is blocked end to end (flat-seam's NOTES): the bottom region (rows <= -1) can be reached from
  the top only through a side piston fired from above and watched from below ((4,0) under the right
  pair, or (-2,0) under the left pair; both cells are free in m7).
* A pulse-driven side piston watched from below fires the watcher TWICE (head in, block arrives); a
  merger (delay-2 repeater + observer) needs a horizontal repeater next to the watcher's output cell,
  and in this geometry that repeater points into a floor-ring park cell (BR's (2,-1) or BL's (0,-1))
  or into a bottom puller.
* So the crossing should be a torch-held right pair (box72's torch link: pistons extended at rest,
  a wall torch at (4,3) on (3,3) feeds (4,2) directly and (4,1) by QC; lever level on (3,3) turns
  the torch off): the observer at (4,0) then fires exactly once per edge.
* The top is cut in two by column x = 1 (P_M, P2, the ceiling ring). The only crossing is row 6
  above P2, and P2's quasi-connectivity makes (0,6), (2,6), (1,7) dangerous: nothing there may emit
  toward x = 1 or be a powered conductor (it would power P2). Observers facing up/down there and
  observers facing away from x = 1 are safe.
* P_M has three pulses at two positions: one emitter at (2,4) or (0,4) (a stone strongly powered, or
  an observer facing x = 1) fires P_M at (1,4) directly AND at (1,3) by quasi-connectivity; the
  cells QC-adjacent to (1,4) are P2's direct neighbours, so they fire P2 too.
* Repeaters, comparators and dust need a sturdy block below: nothing of that kind on the box's
  bottom row (first wiring attempt wip/w0.txt lost its whole row -3 delay line that way).

## Log
* 21:44Z m7 ideal schedule works (P2 retraction must wait for P_M#2 to finish: gap 6, not 4).
* 21:46Z, 22:00Z `door mech-wire` on m7 / the top half alone: no routing (0 of 66 lever spots reach
  every receiver alone).
* 22:31Z wip/w0.txt (bigger box, torch-held right pair, bulb-and-comparator delay at the bottom):
  the bottom row's repeaters/comparator pop off (no support).
