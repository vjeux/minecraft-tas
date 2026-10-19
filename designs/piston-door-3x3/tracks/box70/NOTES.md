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

## 7x8 (56): satwire on pistmodel's exact 3-signal plans (explore/pistmodel fullmech/7x8)

`door satwire <fm-7x8-NNN-groups.ideal> --forbid-ring --palette block,dust,rep,obs,torch,note,bulb,
cmp,lamp,trapdoor,dropper,obsidian,glass --dmax 12 --skip 12` (the mechanism's own 7x8 box, lever
anywhere around it). Every run is UNSAT in 12-40 s, and satwire names the core:

| entry | as planned (1-tick group pulses) | group pulses widened to 2 gt (still `door ideal` WORKS) |
|---|---|---|
| fm-7x8-000 | UNSAT, core close (-1,3) | UNSAT, core close (-1,3) + close (1,4) |
| fm-7x8-004 | UNSAT, core close (-2,1) (the undriven ML puller gets power) | - |
| fm-7x8-012 | UNSAT, core close (-1,3) | UNSAT, core close (1,4) + close (-1,3) |
| fm-7x8-016 | UNSAT, core close (-2,0) | UNSAT, core open (2,-2) + open (1,-1) |
| fm-7x8-020 | UNSAT, core close (-1,-1) | - |
| fm-7x8-024 | UNSAT, core close (2,-1) | - |
| fm-7x8-028 | UNSAT, core open (1,4) | - |

Reading: the 1-tick pulses of the plans are not producible by in-plane wiring (single-piston cores);
with 2 gt pulses the top-elevator entries (000, 012) hit the same wall as m7u: P_TL and P_M must
fire together on the close edge and their only feeds share (0,4) (a powered conductor there also
quasi-powers P_TL); the bottom-elevator entry 016 hits it on BR's puller and the raised P_M.

## Log

* 21:44Z m7 ideal schedule works (P2 retraction must wait for P_M#2 to finish: gap 6, not 4).
* 21:46Z, 22:00Z `door mech-wire` on m7 / the top half alone: no routing (0 of 66 lever spots reach
  every receiver alone).
* 22:31Z wip/w0.txt (bigger box, torch-held right pair, bulb-and-comparator delay at the bottom):
  the bottom row's repeaters/comparator pop off (no support).

## satwire (SAT wiring synthesis, explore/satwire) on m7

Tool changes (this branch, superseded by main's satwire which has both): satwire reads ideal inputs
placed at z = +-1 (the usual `door ideal` placement; it used to see only in-plane redstone blocks, so
most pistons were recorded unpowered), and `--set` / `--sweep name=lo..hi[:step]` over the schedule's
variables. Before main's retiming (`--dmax/--skip` stalls and skips), timing mattered: the solver only
shifted the whole schedule, so every relative offset had to be one a wiring can produce (observer =
2 gt, repeater window 8 gt, P_M's pulse at (1,3) exactly 2 gt after P2's power).

| schedule | box | result |
|---|---|---|
| m7, right pair at 14/16/18, bottom at 24..30 (sweep/a_*) | 7x10 (-2..4, -3..6) | UNSAT (30 s .. 5 min each) |
| m7u (TR up, MR held extended at rest = torch link), bottom at 26 | 7x10 | UNSAT (32 s) |
| top half alone (no bottom pullers), P_M#2 one tick late | 7x4 top (-2..4, 3..6) | UNSAT (4.5 min) |
| top half alone, P_M#2 2 gt after P2, p=4, a=8 | 7x4 top | UNSAT (25 min) |
| top half alone, p=4, a=10..12 | 7x4 top | **SAT** (~15 min): the top IS wirable (lever at (-1,7) outside the box; P2's window from a delay-4 repeater at (0,5) facing west; an observer at (1,6) facing west; note blocks; ~19 blocks) |
| top half alone, p=4, a=10 (re-run, captured) | 7x4 top | **SAT**, 23 wiring blocks, lever at (2,7) on an observer, below |
| top half + right pair, a=10, r=14/18/22 | 7x4 top | UNSAT |
| full m7, a=10, r=16, bottom at b/b+2/b+4 (BR, M_B, BL), b=22..30 | 7x10 | UNSAT x5 |
| full m7, a=10/12, r=16..22, all bottom at b=24..36 | 7x10 | UNSAT (every timing finished so far) |
| m7u (TR up, MR level-held), satwire with retiming (`--dmax 12 --skip 12`, palette + lamp, trapdoor, dropper, obsidian, glass) | 7x10 | UNSAT in 39 s; core: P2, P_TL, P_M in the close trace (with TR's puller at (2,4), P_M's only feeds are (0,4), which also quasi-powers P_TL, and P2's own neighbours) |

The satwire top (rows 3..7 shown, x -3..5; P = mechanism pistons, `:` = swept ring cells, `#` block,
`+` dust, `e/w/u` observers facing east/west/up, `{` delay-2 repeater facing east, `⇒` delay-4
repeater facing west, `N` note block, `L` lever outside the box):

```
  7      L
  6  +e{euww
  5  N⇒#Pw+u
  4  u##P+#+
  3  #P::#e#
  2  P:HHH
```
Notable: the TR -> TL link runs along row 6 through an observer at (1,6) facing east and a repeater
at (0,6) facing EAST (P2's quasi cells are safe as long as nothing there emits toward x = 1), and
an observer at (2,5) facing west watches P2 itself. What does not fit is the right pair on top of
it (UNSAT for every right-pair time tried), so the bottom never gets its trigger in 7x10 with m7.
| top half alone + right pair at r = 10..22 (a = 8) | 7x4 top | UNSAT |

`door topology` (main) on m7 in x -2..4, y -3..6: WIRABLE with the lever in the top component (links:
the right puller (4,1) watched from (4,0), the left puller (-2,1) watched from (-2,0)).

