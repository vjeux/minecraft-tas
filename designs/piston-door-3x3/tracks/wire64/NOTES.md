# Track wire64: a 1-wide (flat), slime-free, FULL seamless + hallway door in an 8x8 box (64b)

Brief (coordinator, 2026-10-18 22:41Z): wire agent families' 8x8-102 (v1 catalog id, commit 6b2d389)
`SE(-3,1) SE(-2,1) SE(-2,0) SD+(-1,3) SD(0,4) SU(1,-2) SD(1,4) SU(2,-2) SD(2,4) SW(4,1)`, box x=-3..4,
y=-2..5 (door at 0..2): the center exits LEFT through the row-1 elevator E3 (-3,1) + L1 (-2,1), ML is
lifted aside by PD (-1,3) (extended at rest, released on open), TL/T/TR up, M/BR down, MR right.
Record to beat: flat-seam e89b, 72b (8x9x1). Box64's notes (tracks/box64/NOTES.md sections 1-5) are
the starting point: its sym-train plan breaks because the lifted-L1 driver (observer (-2,2) facing
west, the only cell that reaches L1 at (-1,1) on both edges) watches E3's driver and so fires twice.

## 1. The E3 window (ideal inputs, ideal/w102w.ideal): WORKS at 32 positions x 2 cycles

Same 13-event train on both edges, but E3 gets ONE LONG pulse [s3, s5) instead of two 2 gt pulses:
it pushes L1 up at s3, stays extended (head at (-2,1)), and its sticky retract at s5 pulls L1 back.
The lifted L1 then needs exactly one pulse per edge, and a driver that watches E3's input sees ONE
rising change per edge. Inputs: redstone blocks behind the plane (z=1), L1-at-rest and PD through
their quasi cells (a direct input to L1 quasi-powers L0 below it) with a glass poke for the update.
- open = close = s1 L1, PD level at lv, E3 window [s3, s5), lifted L1 at s4, s6 L1, 7 toggles at tg
  (close: s6 is a no-op, the R1 toggle repairs ML/C/MR that the lifted L1 shoved east).
- windows (others at s1=2, lv=8, s3=12, s4=14, s5=20, s6=24, tg=28): lv 7..12; s3 9..12; s4 = s3+2 or
  s3+3 (L1 lands at s3+2 and fires on landing if powered); s5 >= s4+5 (L1's retract must be over),
  s5 19..22; s6 = s5+2..s5+8; all 7 toggles together tg >= 24 (R1 after the close's shove; the
  others are free).
- `door ideal --extra-offsets 28`: WORKS, open 31 / close 31.

Planned left core for the window (to prove as a gadget): dust at (-3,3) standing on a waxed copper bulb
at (-3,2) above E3: the dust carries E3's window; the bulb (a conductor) powers E3 for the whole window
and toggles ONCE per window (rising edge only), so the observer at (-2,2) facing west watching the bulb
fires once per edge, 2 gt after s3 = s4. The bulb is lit when open, unlit at rest (one toggle per edge).

## 2. The real left core (ideal/w102-left3.ideal + w102-left2.txt): WORKS at 32 positions x 2 cycles

Real blocks for the whole left core, ideal inputs only for its three trigger cells:
- note block N at (-3,2) above E3, cross dust at (-3,3) standing on N: the dust carries E3's window
  [s3, s5); N (a conductor) powers E3 for the whole window and changes its `powered` state at s3 and s5;
- observer W at (-2,2) facing west watches N: it fires at s3+2 (the lifted L1, landed at s3+2, extends
  at s3+3: s4) and again at s5+2, when E3 has already pulled L1 back (pull started at s5): no race;
- the second W pulse even does s6 on OPEN: L1 lands at rest at s5+2 while W is on, ML at (-1,2) is
  strongly powered by W, so L1 is quasi-powered through (-2,2) as it lands and fires; on close the PD
  head sits at (-1,2) (not a conductor) and nothing happens (s6 is a no-op there anyway).
  So L1 at rest needs ONE pulse per edge (s1), not two.
- emulation detail: an ideal block placed BETWEEN ticks makes W fire one tick too early relative to E3
  (E3 acts in the next tick's block events); real wiring switches the dust in the scheduled phase,
  so the window is fed through a delay-1 repeater behind the plane (w102-left2.txt, z=1) in the test.
- windows (w102-left3, others at s1=2, lv=8, s3=12, s5=20, tg=28): s1 0..3, lv 7..10, s3 10..13,
  s5 >= 19 (up to 30+ tested), all 7 toggles together: tg 14..15 or >= 19.

## 3. Why 8x8-102 stays blocked: the two bottom regions (concrete)

The plane splits into three wiring regions: TOP (20 cells: E3 via N, L1 via (-2,2)/(-2,3), PD, D0-D2,
R1 via (4,2)), BOTTOM-LEFT K0 = (-3,0) (-3,-1) (-2,-1) (-1,-1) (-3..0,-2) + ring (0,-1) (feeds L0 and,
through (0,-2) or the ring block (0,-1), UM), BOTTOM-RIGHT K2 = (3,-1) (4,-1) (3,-2) (4,-2) (4,0) +
ring (3,0) (feeds UB). L0, UM and UB are pull-type pistons: each needs exactly ONE pulse of <= 2 gt per
edge (a longer window pushes the block back on close and the sticky retract pulls it out again:
wire56's memory-window gadget), or three.
- The lever touches one box cell, and the TOP must have it: PD needs a lever-derived LEVEL that moves
  before s3 (no other region can reach PD's cells, and no link reaches the top before s3).
- K0's only link to the top is E3 (watched from (-3,0)). With the window E3 changes twice per edge
  (extend at s3, retract at s5; the landing is swallowed): two pulses 7+ gt apart. Not mergeable (the
  merger takes <= 4 gt), and the two-flip-flop converter (bulb, comparator, bulb, comparator, observer)
  does not fit: comparators need floors, so only row -1 can hold them ((-3,-1) (-2,-1) (-1,-1)), and
  the second comparator would have to read a bulb at (-1,-1) horizontally, whose other side is the
  ring cell (0,-1) (plain blocks only). With E3 pulsed twice instead (s5 = s3+6) the watcher could fire
  three times, but then W's second pulse races E3's pull (box64's blocker) because W watches E3's driver.
- L0 cannot be fed from the top: its quasi cell (-2,1) only borders (-2,2) (= W, which must face west)
  and swept cells. UM's free neighbors are K0 only ((1,-1)'s neighbors are swept or ring).
- K2's only link is R1 (watched from (4,0)): two pulses 4 gt apart per edge (extend, then the landing
  of the retract). The merger needs a delay-2 repeater fed sideways, but the watcher's output points
  down into (4,-1) and the only repeater cell (3,-1) outputs into (2,-1), UB's quasi cell: UB would get
  the merged 4 gt window, which fails on close. Two raw pulses 4 gt apart re-fire UB as it lands.
- Moving the lever next to K0 or K2 leaves the top without PD's early level.
Variants checked in the catalog: BR right (SW(4,0)) removes K2 but not K0; BL down (8x7-080..083,
SU(0,-2) instead of SE(-2,0)) frees (-2,0) and lets K0 reach L1 at rest and, through the unused ring
cell (-1,0), the lifted L1, but U0 (BL's puller) and UM still need single pulses from regions that
only see E3/L1 (even or many-change links) or R0 (two pulses 4 gt apart).
