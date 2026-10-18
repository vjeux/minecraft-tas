# Track box72: flat, slime-free, FULL seamless + hallway door in 8x9 (72) then 7x9 (63)

Coordinator 21:35Z: flat-seam's e89 already made 8x9 = 72 (pulse mergers); this track goes
straight to 7x9 = 63 (x = -2..4: no outer column at all).

## Why 7 wide is a wiring-topology problem (21:40-22:10Z)
In 7 wide the side pullers sit at x = -2 / x = 4 and their gaps at x = -1 / x = 3, so:
* Every side cuts the box at y = 1: ML's puller (-2,1) + its gap (-1,1) (ML can only leave
  sideways; a vertical exit needs an elevator). Same on the right.
* The center column is blocked top to bottom by the elevator, the T / M pullers and their gaps;
  in 9 rows only ONE end gets a crossing row (flat43: row 6 over B, bottom rows -2..-1 split by
  P_M (1,-2); cd19/e89: row -4 under B', top rows 3..4 split by the T puller (1,4)).
* So the box is >= 3 regions: flat43-type {TOP, BL-quad, BR-quad}, cd19-type {BOTTOM, TL, TR}.
  The lever sits in one; the others need LINKS (motion one region causes and another observes):
  - side link: ML's puller watched from (-2,0) or (-2,2): needs that cell free, i.e. that corner
    leaves vertically (BL down via (0,-2) / TL up via (0,4)). From a region you can fire two
    pistons of a stack (observer trick from above) but only ONE from below.
  - center link: a center piston fed from one quad and watched from the other (T puller in e89,
    P_M in flat43); needs BOTH (0,y) and (2,y) beside it free.
  - the two always collide: flat43's side link puts BL's puller at (0,-2) next to P_M (P_M then
    has no feeder on that side), cd19's side link puts TL's puller at (0,4) next to the T puller.
    Both side links at once leave no park cell for C / T. => none of the e89 / f43-89 / families
    entries can be wired in 7x9 with pull-type links and mergers alone.

## New gadget: torch link (level-driven link piston), designs/gadgets/torch-link.txt + .drive
A sticky piston held EXTENDED at rest by a redstone torch (head attached to its door block),
torch on a block the lever powers: lever on -> torch off -> piston retracts (pulls its block),
lever off -> extends (pushes it back). An observer watching it fires exactly ONCE per edge
(measured: t4-5 on open, t16-17 on close). So a level-driven side middle puller is a parity-clean
link (no merger). Caution seen: a block strongly powered by that observer must not sit next to
the piston's QC cell (my first rig re-powered it through a lamp).

## Status
Looking for a 7x9 layout with one extra link type (torch link, BUD release, or a lever outside
the box on a region boundary).

## 7x10 = 70 (flat43 center + left torch link): mechanism OK, top wiring open (22:00-22:50Z)
* [wip/m710.txt](wip/m710.txt) + [wip/m710c.ideal](wip/m710c.ideal) / [wip/m710s.ideal](wip/m710s.ideal):
  x -2..4, y -3..6. Left: TL up (puller (0,4)), ML torch-held (-2,1) (torch (-2,0) on the
  lever's block (-2,-1)), BL down (puller (0,-2)); center flat43 (A (1,4), B (1,5), P_T (3,3),
  P_M (1,-2) fed from row -3); right 3-stack. WORKS with ideal inputs, A fired only through an
  observer (2,4) facing east (a2 by QC through B's head). One symmetric train exists: a1 0..7,
  pt 7..12, b1 14..15, a2 b1+1..2, b2 22..24, a3 = b2+1.., P_M on close after a3, TL / BL /
  right side any time.
* The catch: the link side's top corner leaves UP, so its puller (0,4) sits next to A; A's feeder
  and P_T are then on the far side (x >= 2), while the top's only trigger (the link watcher
  (-2,2)) is on the near side. The only top crossing is row 6 at (1,6) = B's QC cell: anything
  crossing there arms B. B's window + A's a2/a3 cross fine (f43 style: B window repeater into
  (1,6), an observer at (1,6) watching it, its output lights a dust dot on a stone at (2,4) ->
  A on each window edge by onPlace when A lands; checked in the simulator, wip/v1.txt), but a1
  and pt need an EARLY far-side event and every early crossing either fires B or collides with
  A's window pulses. Not solved yet.

## M2: flat43 mirrored + BL down (side-pair link), 22:55-23:20Z
* [wip/m2.txt](wip/m2.txt) + [wip/m2.ideal](wip/m2.ideal): pist19r (T parks LEFT via (-1,3)) with
  BL leaving DOWN (puller (0,-2)); left side = pair (-2,1),(-2,2) fired from above; (-2,0) free.
  WORKS with ideal inputs (39/39).
* [wip/m2w.txt](wip/m2w.txt): its TOP hand-wired inside x -2..4 (flat43-full mirrored with
  `door turn --turns 0 --mirror`): the lever OUTSIDE the box at (-3,4) (volume excludes it) on
  the observer (-2,4) that watches it; observer (-2,3) facing up fires TL + ML (observer trick);
  far pistons TR/MR fired once per edge by a waxed copper bulb (3,5) under the B-window dust
  (3,6), read by a comparator (4,5) (an observer on the bulb itself fires twice: its `powered`
  property follows the 8 gt window), observer (4,4) on the comparator -> stone (4,3). Checked
  with `door ideal` + ideal bottom inputs: everything opens except M (see below).
* The bottom is the problem: the side-pair link gives the bottom TWO pulses (ML's puller is
  pull-type: extend + landing, 4 gt apart), so it needs a merger (delay-2 repeater + observer),
  and in rows -3..-1 every repeater spot on the left outputs into BL's puller or its gap (QC):
  a 4 gt pulse breaks a pull-type piston on close (push, then pull back). The right half is
  reachable only along row -3. P_M's flat43 trigger (observer on BR's arrival cell -> lamp ->
  observer) also double-fires P_M here (measured: lit at t40, dark at t50). So M2 needs a 4th
  bottom row (7x11 = 77, worse than e89's 72).
* satwire: both plans UNSAT in 7x10 (m710b 29 s, m2 454 s). satwire does model observers
  watching mechanism cells (`mchg`), so the UNSAT is real for those exact schedules; but every
  piston's power must match the schedule tick for tick (only a global offset), so a link +
  merger chain has to land on the scheduled ticks exactly.

## Bottom parity breakers tried on paper for m2 in 7x10 (23:20Z), none fits in rows -3..-1
* delay-2 repeater merger: every repeater spot on the left half outputs into BL's puller or its
  gap (a 4 gt pulse breaks a pull-type piston on close); on the right half (reached only along
  row -3) the merged pulse can fire BR's puller (repeater (4,-2) facing west fed by an observer
  (3,-2) watching an observer (2,-2) that watches the row -3 chain, merger observer (4,-1) facing
  down) but cannot get back west to BL's puller and P_M (row -3 is the only lane).
* bulb + comparator + bulb + comparator (2 -> 1 events): comparators need a floor, row -3 has none.
* making both bottom corners leave DOWN (P_M then fed from (1,-3)) frees (4,0) too, but the
  merger has the same output problem on the right.
* a 0/1 gt pulse on ML's puller would make its watcher fire once (all base changes inside the
  observer's busy window), but needs 0-tick machinery.
=> next: satwire with the hand-wired top frozen (`--fixed`, in progress by agent satwire) and
   time-warp, or a 7x11 = 77 build of m2 (row -4 gives the merger a floor; not better than 72).
