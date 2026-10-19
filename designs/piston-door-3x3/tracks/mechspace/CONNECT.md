# `door connect` (wire64b, main) on the new mechanisms

Run: `mctas door connect <tsv> --boxes 7x8,7x9` (connectivity + event order, a necessary condition;
caps on 12 pistons). WIRABLE placements (box, x and y ranges in door coordinates):

| mechanism | 7x8 (x -2..4, y -2..5) | 7x9 (y -2..6) | 7x9 (y -3..5) |
|---|---|---|---|
| 7x8-c2e-001 | no | no | no |
| 7x8-c2e-002 | no | WIRABLE | no |
| 7x8-c2e-003 | no | no | no |
| 7x8-c2e-004 | no | WIRABLE | no |
| 7x8-t1-001 | WIRABLE | WIRABLE | WIRABLE |
| 7x8-t1-002 | WIRABLE | WIRABLE | WIRABLE |
| 7x8-t1-003 .. 016 | no (cap) | no (cap) | WIRABLE |

("no" here = UNDECIDED with a cap hit on some lever region, or a region never woken; see the run
log.) Of the old catalog only 7x8-014 / 015 passed connect in 7x8 (coordinator).
