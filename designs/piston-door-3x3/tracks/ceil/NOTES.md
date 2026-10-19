# Track ceil: the e88 ceiling-held top on other top-row-up mechanisms (2026-10-19)

Brief (coordinator 10:08Z): e88-t7b (63b = 7x9, 43 blocks) rests on ONE lever, ON at rest, standing
on top of the middle ceiling puller (1,4): it powers it directly and the corner ceiling pullers (0,4)
(2,4) by QC, so the whole top row (TL TM TR up into the ceiling frame row) needs zero wiring and both
lever edges give clean single pulses (observers watching where the top corners land). Apply it to
every other mechanism whose top row goes up: more 7x8 / 7x9 / 8x8 doors.

## 1. Which catalog mechanisms qualify (top row up: SD(0,4) SD(1,4) SD(2,4), no (x,5) elevator)
- mechspace n79 (7x9): none (TL/TR go sideways or the middle column is an elevator SD(1,4)+SD(1,5)).
- mechspace su-layouts / su8 / c78t: only sl-010/011, sm-002 (SD(2,5) elevator, oc plans broken).
- families CATALOG.tsv: 7x8-028/030 (= t7b's mechanism: agent t78 does it), 7x8-035/036 (BR DOWN),
  the 8x7 family 070-082 (left: a double horizontal puller SE(-3,1)+SE(-2,1) pulls C out left),
  8x8-128, 143, 147, 153, 156.
- ring56 jf (layout J) = t7b's mechanism mirrored (BM into the right floor corner): not new.

## 2. Ceiling-held + everything else ideal: `door ideal-rs` (16 positions x 2 cycles), frame 12/12
The catalog plan with the three top-puller inputs removed, pullers extended at rest (heads in row 3),
lever (1,5) ON on the TM puller (`use` at both edges), static stones in the frame cells left empty:

| id | from | box (mech) | static stones | ideal-rs |
|---|---|---|---|---|
| c035 | 7x8-035 | 7x8 (y -3..4) | (-1,2) (3,2) (3,0) | WORKS 33/15 |
| c070 | 8x7-070 | 8x7 (x -3..4, y -2..4) | (-1,2) (3,2) (2,-1) | WORKS 33/15 |
| c074 | 8x7-074 | 8x7 | (3,2) (-1,0) (2,-1) | WORKS 33/15 |
| c077 | 8x7-077 | 8x7 | (3,2) (0,-1) (2,-1) | WORKS 33/21 |
| c079 | 8x7-079 | 8x7 | (-1,2) (3,2) (3,0) | WORKS 33/15 |
| c081 | 8x7-081 | 8x7 | (3,2) (-1,0) (3,0) | WORKS 33/15 |
| c128 | 8x8-128 | 8x8 | (3,2) (3,0) (0,-1) | WORKS 39/21 |
| c143 | 8x8-143 | 8x8 | (-1,2) (3,2) (2,-1) | WORKS 33/27 |
| c147 | 8x8-147 | 8x8 | (-1,2) (3,2) (3,0) | WORKS 33/27 |
| c153/c156 | 8x8-153/156 | 8x8 | | BROKEN (TL's puller fires twice in the catalog plan) |

Files: wip/c<id>.txt (design), ideal/c<id>.ideal (plan). The 8x7 mechanisms would be 56 like 7x8.

## 3. Wiring: what blocks c035 and the 8x7 family (11:15Z)
Windows (`door ideal-rs --sweep`, real top + t7b's wall chains, c035w / c070w):
- c035: BR's down-puller SU(2,-2) is timing-free on both edges (2 gt pulse, spit on close); the bottom
  is t7b's elevator (A pulse 0, P_M, B, A-high, B, A-low) and can start at any time after the walls.
  BLOCKED: A's only free feed is (0,-2) ((2,-2) is SU(2,-2)), and the BM gate (A-high / A-low on OPEN
  only, through BM parked at (0,-1)) also needs (0,-2) (facing down). BR's park (2,-1) is next to A's
  QC cell, but its only feed (3,-1) also QC-fires SU(2,-2), which shoves BR back up whenever BR is
  parked. With the ceiling, A-high on close is harmless (the push hits the TM puller's head), but A-low
  on close pulls BM down: the catalog's symmetric train fails (`ideal/c035s.ideal`).
- 8x7 family (c070): BL/BM down and MR+BR are timing-free single pulses (BM's puller from (2,-2) or the
  static F2 stone: no BM-puller wall in c070); U is a both-edge pulse; E1 comes from t7b's TL chain.
  The left double puller needs ONE open-only event: either D (push E, pull E back) on OPEN only with E2
  on both edges (`ideal/c070h.ideal`: D a 10-gt hold at 17..20, M a 9-12 gt hold at 13..16), or D on
  BOTH edges (its push is then the close's ML+C push) with M and E2 on OPEN only
  (`ideal/c070d.ideal`: D taps 15..21 / 25..36 both edges, M 20..30, E2 >= 37 open only). M is gated
  for free by ML parked at (-1,0) (a source at (-2,0) emitting east reaches E at (-1,1) only while
  open). But (-2,0) is the ONLY free cell that sees an edge difference on the left (an observer facing
  east there gives 2 pulses on open, 1 on close, out into (-3,0) = D's feed), so it can gate M OR feed
  D, never both; E2 has no gate (no door block parks next to E's feeds). pockets k=4 (exhaustive) in
  the 8x7 box and the 8x8 box (y -3..4, 365M fillings): drivers for U, E2, BL, BM (2-3 blocks each),
  never the D / M holds.
- c074 (ML parks at (-1,2) via SD(-1,3)): ML at (-1,2) gates E2 (QC through (-2,2)), tested in
  `ideal/c074g.ideal` (an observer (-2,2) facing west, toggled by the plan: WORKS only at gm 18 ge 33),
  but that observer also QC-powers E at (-1,1) directly (M leaks on close), and (-2,2) is again the
  only edge-sensitive cell.

## 4. The blocked-push watcher: D on open only for free (11:30Z)
- c077w (`wip/c077w.txt`, `ideal/c077w.ideal`): an observer (-2,2) facing EAST watches ML's park
  (-1,2) and powers the stone (-3,2) on D. Open: S's pull makes 3 changes in (-1,2) -> 2 pulses ->
  D pushes E (+M) and pulls it back. Close: S's push makes the pulse arrive while ML is still a
  moving block in D's push line -> D's push is BLOCKED (moving blocks are immovable) -> D never moves
  on close. M gated by BL parked at (-1,0) (observer (-1,-1) facing down; BL is on its puller's FRONT
  face, so the puller ignores it). E through an observer (-2,3) facing up (QC + front update of E).
  `door ideal-rs` WORKS with both-edge ideal inputs only: BL-left [0, s-1], E1 [2, s-2], S at s, the
  M gate EXACTLY s+4 (E powered on landing at (-1,1)), E2 >= s+11, BM and MR/BR free (s >= 8).
- c070x (`wip/c070x.txt`, `ideal/c070x.ideal`): the same trick from the bottom (observer (-2,0)
  facing east on ML's park (-1,0) -> stone (-3,0) under D; U's pull vs push). WORKS with both-edge
  assists: U at u, M at exactly u+3 (E on landing; ungated since E never reaches (-1,1) on close),
  E2 >= u+10, BL/BM down free.
- Both confirmed robust: `door ideal-rs --extra-offsets 28 --cycles 3` WORKS (32 positions x 3
  cycles) and the real game agrees (`door ideal`, JVM, 4 positions x 2 cycles: WORKS) for c077w
  (s 8, gm 12) and c070x: moving blocks really do block the push in the game.
- What is left is timing, not logic: M must be powered on the exact tick E lands at (-1,1) (D's two
  taps are the watcher's double pulse, 4 gt apart). pockets k=4 on c070x: BL, BM, E2-open drivable;
  U and M have single-tick windows ([+0,+0]) and no filling. M's feeds: c070x has none left in 8x7
  ((-1,2)'s free neighbours hold the E1 chain); c077's M gate needs a bottom signal timed to S, and
  c077's top-left cannot feed both E1 and S in 8x7 (S's only feeds are (-1,4) (a conductor there
  powers the TL puller) and QC through (-2,4), and E1's observer (-2,3) can only watch (-2,4), so E1
  always comes 2 gt AFTER S). An 8x8 box with row 5 (y -2..5) frees (-1,4) for an observer facing
  up (S) and gives a lever watcher at (0,5).
- Tried after that (all fail in redsim): c070y (M from an observer chain watching D: M lands 2 gt
  late, D's pull hits E extended); c078w (`ideal/c078w.ideal`, 8x7-078 = c077 with SD+(-1,3)
  extended at rest, lever-held through a row-5 repeater + dust on glass in an 8x8): S+ moves ONCE
  per edge, so the ML-park watcher fires once and D gets its push but never its pull-back. In 8x8
  y -2..5 the E1-before-S order still fails for c077 (every early signal enters at (-1,5), which is
  S's watched cell, before it can reach (-2,4)).

## 5. Next ideas
- A 10-gt D hold (bridge: D works as one hold) instead of the watcher's two taps would widen M's
  window from 1 tick to ~8; it needs a pulse stretcher on one of D's two feed cells ((-3,0) or
  (-3,2)), which only take observers/conductors from the watcher's side.
- Feed the bottom-left M gate (c077) and the top-left S from two deterministic chains of matched
  length (top: TL-puller watcher; bottom: right chain -> BR-puller watcher -> BM-down -> observer
  (0,-2) east -> note block (-1,-2) -> the gate), once the top-left can order E1 before S.
- The blocked-push watcher is general: any piston whose job must happen on one edge only can be
  driven by a watcher whose pulse, on the other edge, arrives while a door block is still moving
  through its push line.
