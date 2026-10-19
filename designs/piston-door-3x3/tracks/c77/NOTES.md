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
