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

## New gadget: torch link (level-driven link piston), wip/torch-link.txt + .drive
A sticky piston held EXTENDED at rest by a redstone torch (head attached to its door block),
torch on a block the lever powers: lever on -> torch off -> piston retracts (pulls its block),
lever off -> extends (pushes it back). An observer watching it fires exactly ONCE per edge
(measured: t4-5 on open, t16-17 on close). So a level-driven side middle puller is a parity-clean
link (no merger). Caution seen: a block strongly powered by that observer must not sit next to
the piston's QC cell (my first rig re-powered it through a lamp).

## Status
Looking for a 7x9 layout with one extra link type (torch link, BUD release, or a lever outside
the box on a region boundary).
