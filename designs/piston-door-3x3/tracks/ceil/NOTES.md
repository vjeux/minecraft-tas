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
