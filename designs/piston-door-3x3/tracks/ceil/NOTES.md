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
