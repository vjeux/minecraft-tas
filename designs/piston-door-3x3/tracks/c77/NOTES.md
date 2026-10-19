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
