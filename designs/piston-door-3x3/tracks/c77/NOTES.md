# Track c77: ceil's 8x7 family (8 wide x 7 tall = 56) wired: c074 and c081 (2026-10-19)

Coordinator: ceil keeps c077/c070; c77 takes c074 and c081 (ceil's lever-held ceiling top; D =
SE(-3,1) driven by ceil's ML-park watcher: observer (-2,2) facing east -> stone (-3,2) over D).

## c081: BM's puller SU(1,-2) is unfeedable inside 8x7
Its neighbours are SU(0,-2), SU(2,-2) (pistons are not conductors), (1,-3) (outside the box) and its
front; its QC cell (1,-1) touches only the floor parks (0,-1)/(2,-1) (air at rest, door blocks when
open) and BM. Nothing can fire it on open inside the box (same verdict as bridge's c079). Not pursued.

## c074 with D real (wip/c074r1.txt, ideal/c074r1.ideal)
c074 + bridge's right side (observer (3,3) W on TR's park -> note block (4,3) -> observer (4,2) up:
MR direct, BR by QC, t5 both edges; BR+MR work at ANY common tick, 2601/2601) + ceil's o/stone.
ideal-rs WORKS with E1 (plan 0..3 both edges), TLc (plan T), M (plan T+3 EXACTLY), E2 (open >= 16,
close >= 13: one both-edge pulse works), BL+BM (any tick both edges) ideal.
- o fires twice on open (TLc's head enters (-1,2) at A, ML lands at A+4): D pushes E at A+2 (spit at
  A+4), D pulls E back at A+8 (landing fire at A+7). M (E spit-placed at (-1,1) at A+4) must be powered
  then and unpowered at A+5 (its base must land before D's pull): a 1-tick window.
- On close o's first pulse finds ML moving in D's line (D blocked) and fires E by QC through (-3,2)
  when TLc's spit places ML at (-1,1) (a free E push); E retracts when o drops, so E2 must come >= 13.
- PARITY: TLc driven by a top-derived observer acts at an odd tick A; M's observer must turn on at A+3
  (even), so M's chain needs one landing-triggered piston (acts the tick after a BE-phase landing).

## M solved twice, E1 + TLc not (12:00Z-12:50Z)
- M by sculk (wip/c074s1.txt): calibrated sensor (-1,-2) facing west (back on BL's puller) hears the
  lever at t7, observer (-1,-1) facing down -> static (-1,0): M fires exactly when D spits E at A+4 if
  TLc acts at A = 6 (plan tlo 5). ideal-rs WORKS with E1, TLc, E2, BL, BM ideal.
- D held by a PLAIN sculk sensor (wip/c074d2.txt, ideal/c074d3.ideal): sensor (-3,0) under D hears the
  lever at t6 and holds D 30 gt; o's first pulse updates D once TLc has lifted ML, D pushes E and HOLDS
  it (no spit), o's second pulse is swallowed, D pulls E back at t37. M then works at ANY plan tick
  9..30 (any both-edge pulse: harmless on close), E2 one both-edge pulse >= 37, E1/TLc pairs (plan,
  same on both edges) (1,4) (1,5) (2,5) (2,6) (3,6) (3,7) (4,7) (4,8) (5,8); BL/BM any.
- E1 + TLc in the 8x7 top-left: impossible so far. E's only feeds are (-2,3) (must emit DOWN only: a
  conductor there powers TLc east of it) and (-2,0) (bottom-left: no source before t6). The only early
  top source is X = observer (-1,4) facing east on TL's puller (on t3, both edges); X outputs into
  (-2,4), and anything there that emits east QC-arms TLc (which TL's t3 landing then fires at t4).
  X -> TRAPDOOR (-2,4) (non-conductor) -> observer (-2,3) facing up gives a clean E1 at t5 (plan 4),
  but then TLc has no feed left ((-2,3), (-1,4), (-2,4) all taken). Exhaustive: pockets k=4 full
  palette on the top-left (c074s1 plan, 89M fillings): only E2c; ideal-rs --vary 21 states^5 (4.1M)
  on the top-left with E1 and TLc real (c074d3 plan): 0.
- Next: the same with an 8th row on top (8x8 = 64, box y -2..5): pockets k=3 running.

## RESULT (13:12Z): c074-64b-42 = 8x8 (64b), 42 blocks, sculk, slime-free, flat, FULL seamless, rotation-safe
`c074-64b-42.txt` (= BEST.txt, archived in tracks/c77/archive/): `door verify --extra-offsets 28` WORKS
(32 positions x 3 cycles, all 4 turns), open 42 / close 42; `door properties`: FULL seamless, dentless,
volume 64 (8x8x1). Box x -3..4, y -2..5 (the lever (1,5) is excluded; row 5 holds the dust and repeater).
- Top: ceil/e88's lever-held ceiling (lever ON on the TM puller (1,4), QC for TL/TR pullers).
- TLc (-1,3): the dust (0,5) on the TL puller, lit by the lever, feeds a delay-4 repeater (-1,5) facing
  west; observer (-1,4) facing up on the repeater fires TLc once per edge (t10).
- E (-2,1): plain sculk sensor (-3,4) (hears the lever, t4) lights a lamp (-2,4); observer (-2,3) facing
  up on the lamp QC-fires E through (-2,2) at t6 (E1) and when the lamp goes dark at t40 (E2): 3 blocks.
- D (-3,1): plain sculk sensor (-3,0) under D (hears the lever, t6) holds D 30 gt; ceil's ML-park
  watcher (observer (-2,2) facing east -> stone (-3,2)) updates D once TLc lifted ML, so D pushes E to
  (-1,1) and HOLDS it (no spit), then pulls it back when the sensor ends (t37). On close D's push meets
  ML moving in its line (ceil's trick). This turns ceil's 1-tick M window into plan 13..30.
- M + BL: one 4-block pockets filling (bottom-left); BM: observer (3,-2) E, sticky (4,-2) W, observer
  (4,-1) up; MR + BR: bridge's observer (3,3) W, note block (4,3), observer (4,2) up.
- Found by: c074 plan with D real -> M by sculk -> D hold sensor -> pockets k=3 in 8x8 (E1+E2, TLc,
  M, BM fillings) -> repeater delay 3 -> 4 (TLc after E1) -> pockets k=4 on the bottom-left (M + BL).

## 8x7-072 (13:28Z-13:40Z): dead in 8x7 (partials/c072.md)
U (-1,-1) is extended at rest and needs a placement-safe LEVEL source (torch / powered repeater /
redstone block): a comparator straight into U outputs 0 at placement (block entity), so U drops while
settling (redsim, wip/c072u.txt). U's feeds are boxed in ((-2,-1) diode from (-3,-1) only; (-1,-2)
also powers BL's puller; QC via (-2,0) also powers E), row -2 has no floor for diodes, and the free
top-left cannot pass a level down past D/E/(-1,2). Same for 076 (TLc level-held).

## e78 STONE variant (13:32Z-, coordinator): pockets2's partial with only B' (1,-3) ideal (e78s/kb1.txt)
- Under main's TILE phase (de6ecbce): B' works with THREE same-tick pulses on both edges {17, 26..32,
  +5..9} (e78s/kb1p3.ideal, 16 positions x 3 cycles); 2 pulses per edge: open {17..22, 28..60}, close
  {17, 23..24} / {18, 20..25, 27} / {20, 22..25, 27} (no common pair); pockets3: one hold per edge also
  works (open [18,28) = a calibrated sensor's 10 gt, or [23,46), [24,44); close any hold from 27..45
  except 17 gt) - a calibrated sensor waking at t18 on open and t27..45 on close would do it.
- Why edge-asymmetric: on close, B2 must come before the left calibrated sensor re-arms (t27): its echo
  fires A' again; on open that echo IS A3.
- B''s clean feeds: (0,-3) (observer facing west, or a conductor fed from (-1,-3)) and (2,-3) (only an
  observer facing east: the K-chain observer (3,-3) faces east and outputs into (2,-3), so a conductor
  there fires B' at t9 with A1 and lifts A' early); (0,-2)/(2,-2) also power A' (only safe while A' is
  lifted, t22..32).
- Screens: ideal-rs --vary 15 states^6 on the 6 free cells, B' fully real: 0 of 11.4M; B1 real with B2
  ideal: 0 of 933k; pockets3: k=4 full palette (62 states) with the 3-pulse plan: 0 of 20.5M; door fill
  k<=4 (45 states incl. pistons/sculk): 0 of 17.6M; k=5 running (pockets3).
