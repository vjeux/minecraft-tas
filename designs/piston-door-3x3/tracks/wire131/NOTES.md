# Track wire131: wire 8x8-131 (wire64's f131L/f131M) as a 7 x 8 = 56b flat, slime-free, FULL + hall door

Brief (coordinator, 2026-10-19 01:39Z): continue wire64's deepest lead, the mechanism 8x8-131 in the 7x8 box
x -2..4, y -2..5 (door at 0..2), in its level-held variants, region by region with ideal inputs.

## 1. Windows of f131M, every input on its own variable (p131M.ideal)

`door ideal p131M.ideal --independent --sweep ...` (others at the plan's values; o = open, c = close; a/b/c = D1's
three shots, px = X, e1/e2 = DE's pulses):
- open: a 0..1 (px 6), px 5..12, e1 6..17, b 13..21, e2 21..26, c 28..44: px >= a+5, e1 >= px, b in [e1+1, e2-3],
  e2 in [b+3, c-4].
- close: a 0..6, px 0..9, e1 9..16, b 14..21, e2 21..29 and 32..34, c 25..36 (the close's third D1 shot is a no-op).
- FREE on both edges (0..44): L0, D2, the R pair level (4,2); UM free on open, close >= 30 (after the no-op shot c:
  that shot's reach-through would pull T back up after UM's push).

## 2. DE as ONE window instead of two pulses (p131M-win.ideal): WORKS at 32 positions x 2 cycles

DE held from e1 to e2+2 (pushes D1 down at e1, stays extended while D1 shoots lifted, pulls D1 back at the end).
Windows: open e1 6..16, release 23..28; close e1 9..16, release 23..36+; b 14..21 both. satwire on this plan
(28 free cells, full palette): UNSAT in 20 s (encoding/plan-relative, see coverage.md).

## 3. f131X: EVERY puller level-held (f131X.txt + p131X-win.ideal): WORKS at 32 positions x 2 cycles

L0+ (-2,0) [held from (-2,-1)], D2+ (2,4) [head (2,3) at rest, held from (3,4)], UM+ (1,-2) [head (1,-1) at rest, held
from (2,-2)] join wire64's L1+/L2+/P3+ (X (-2,3)) and R1+/R0+ ((4,2)). Only D1 and DE stay pulse/window pistons.
Every new level is FREE on both edges (0..44) except UM+ on close (>= 30, after D1's no-op shot).
Why: level pistons move once per edge, so an observer watching one is an odd (parity-clean) link, and a level only
needs one switch per edge at any time.

Variant f131R (TR leaves RIGHT, R2+ (4,2) + holder (4,3), no D2) also WORKS (32 x 2), but it cuts the only
top -> bottom link (R0+ is quasi-held through R1+'s cell from (4,2) in f131X; with R2+ at (4,2) R0+ can only be
held from (4,-1)), so the bottom half could never see the lever. Kept f131X.

## 4. The bottom half wired (f131X-brbl.txt + p131X-brbl.ideal): WORKS at 32 positions x 2 cycles

Real blocks for BL and BR, driven only by the R pair's level at (4,2) (ideal: a redstone block, open any time,
close >= 26):
- BR (3 blocks): observer (4,-1) facing up watches R0+ (one change per edge) and pulses the pull-type sticky
  piston (4,-2) facing west, which shuttles a REDSTONE BLOCK between (2,-2) (rest: holds UM+) and (3,-2) (open).
  A pull-type piston + redstone block is a T flip-flop with a LEVEL output: one pulse per edge -> one switch.
- BL (4 blocks): observer (0,-2) facing east watches UM+ (one change per edge) -> stone (-1,-2) -> note block
  (-2,-2) (weakly powered: its `powered` flips) -> observer (-2,-1) facing down -> L0 back to a pull-type pulse
  piston (-2,0) (one pulse per edge).
Left: TR (D2+ and the R pair's level from DE's motion, the only TL -> TR link) and TL (lever, D1 x3, DE window, X).

## 5. The R pair's level cannot be made in TR: the holder's switch leaks into R1+'s quasi cell (simulated)

R1+ (4,1) and R0+ (4,0) are both held from (4,2) (R1+ direct, R0+ by QC through R1+'s cell): the ONLY
top -> bottom link of the 131 family in 7x8 (BR can watch only R0, BR's park (3,0), UM, and R0 is the only one TR
can drive). A level at (4,2) must be ON at rest and OFF when open with NOTHING around (4,2) emitting when open,
because R1+'s quasi cell is (4,2) (neighbors (3,2), (4,3), (5,2)):
- wall torch at (4,2) on (3,2): its input (3,2) must be powered when open, and a powered (3,2) quasi-powers R1+.
  f131X-trtest.txt + p131X-trtest.ideal (torch (4,2) on stone (3,2), cross dust (3,3) fed from behind at open,
  D2+'s holder moved off-plane): `door ideal`: "not open: [2,1,0] is stone", R1+ never retracts. As predicted.
- conductor (4,2) + dust (4,3) on it: the dust's source must sit at (4,4) or (3,3); a torch there has its input at
  (3,4) (next to D2+ (2,4): D2+ powered when open) or (3,2) (the same leak); a redstone block at (4,4)/(3,3) has no
  free cell to move to; (3,3) and (4,3) themselves are R1+'s quasi neighbors.
- redstone block at (4,2): it can only leave to (4,3) (quasi-powers R1+; R1+ lands after it and re-extends) or
  (3,2) (a ring cell: circuitry in S when open, not FULL).
- comparators in TR output only into x=2 cells (D2+, (2,5), TR's park, TR's door cell).
So R1+/R0+ as levels are unwirable in 7x8; as pull-type pulse pistons, R0's pulse gives BR's watchers an EVEN count
(the (4,-1) watcher fires at t+2 and t+6 on both edges; the park watcher (3,-1) fires twice on open, once on close,
and its output cell (3,-2) cannot feed any flip-flop piston that holds UM+). Same structural leak on the left:
X's dust (-2,4) needs a source at (-2,5) or (-1,4) whose input ((-1,5), (0,4), the lever's support) quasi-powers P3+
via (-1,4) or feeds the dust; a comparator reading a lit bulb fits only as (-1,5) -> (-2,5) with the bulb at (0,5),
whose toggling pulse also powers DE (e1 < px, fails on both edges). This is consistent with satwire's UNSAT cores
(X / L2+ on the left, wire64). Bound: in-plane palette (dust, torches, repeaters, comparators, observers, bulbs,
note blocks, redstone blocks moved by pistons), 7x8 box x -2..4, y -2..5, mechanism 8x8-131 with level or pull-type
side pullers, D2 up, MR/BR right.
