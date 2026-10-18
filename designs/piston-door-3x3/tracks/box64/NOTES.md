# Track box64: a 1-wide (flat) FULL-seamless door in an 8x8 (or 9x7 / 7x9) box

Goal (vjeux 20:20Z / coordinator): flat (thick<=1), volume <= 64, FULL seamless + hallway
(`--seamless full+hall`), slime-free preferred. Starting points: seamless's flat43-full (9x9 = 81,
FULL+hall) and flat-b-v7-full (8x10 = 80, FULL+hall).

## Results so far
- `resynth --fit=-2,-2,0:5,5,0 --seamless full+hall --extra 3 --margin 2` on flat43-full: nothing
  (148 groups; the row-6 crossing cannot be re-grown locally). logs/fit88a.log.

## The flat43 mechanism fits 8x8 mechanically (ideal inputs, all in the box)
`m19-88.ideal` (flat track's m19 with every input inside x=-2..5, y=-2..5): a1 and a3 from (2,4),
P_T from (3,4), B from a redstone block at (2,5) (B's side: A is quasi-powered through (1,5) but
never updated, so it stays put), a2 from (0,3), M from (2,-2), left column from (-2,3) (L2 direct,
L1 by QC + the update when L2 moves) and (-2,-1), right column from (5,2), (5,1), (4,-1):
WORKS at 32 positions x 2 cycles. Windows (a1=2): pt 6..9, b1 10..12 (a2 = b1+2..3),
b2 19..20 (a3 = b2+2..4), M >= 22, L and R anywhere. So "B can only be fed through row 6"
is not true: a conductor next to B, powered by a repeater/observer that does not update A, works.

## Why 8x8 is hard: the plane splits into regions
In one plane, the door, its swept ring cells and the pistons cut the box into regions that can only
talk through the mechanism (observers watching pistons or door blocks) or quasi-connectivity:
- flat43 in x=-2..5: RIGHT (x=5 column joins top and bottom), TOP-LEFT (L2/L1 feed (-2,3), A's and
  B's left), BOTTOM-LEFT (L0 feed (-2,-1)). TL only sees A and B; BL only P_M and L0.
- flat43 in 7x9 (x=-2..4, row 6 kept): both bottom corners (L0's and R0's only feeds) are unreachable.
- flat-b (center down) in 8 rows: the top-right (P_TR, R1) or top-left (P_T, L2) region is
  isolated, because P_T's quasi cell (1,5) is outside.
- Watching a piston or a pull-type gap cell double-fires (the retraction finishes in the block
  entity phase, after the watcher's off tick); watching B (an elevator) gives exactly two clean
  pulses (b1+2, b2+2). A door column needs an odd number of short pulses per flip.
- 2 -> 1 pulse: a delay-3 repeater merges B-watcher's two pulses into one, a waxed copper bulb
  toggles once on it, an observer on the bulb gives one pulse per flip.

## Current direction: mechanism M2 in x=-3..4, y=-2..5
Left 3-stack (fed from x=-3), T parked LEFT at (0,3) by a parker (-1,3) facing east, A/B elevator,
P_M (1,-2), right PAIR (4,0..1) + TR pulled up by (2,4) (both fed from (4,2)/(3,4) in the small
top-right region, which gets one pulse per flip from the B-watcher (2,5) + repeater + bulb).
