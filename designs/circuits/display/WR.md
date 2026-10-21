# Study 15: 7-segment display decoders (WR board)

ONE design: a 4-bit value (four input lines a0..a3) lights the segments a..g of its hex digit,
0-9 then A b C d E F (`compute::SEGMENTS`: a top, b upper right, c lower right, d bottom, e lower
left, f upper left, g middle; 6 and 9 with their tails, 7 without f).

## The judge (`circuit: seven-segment 4`, a study 12 compute op)
- Ports: `port input.a0..a3` (plain inputs, R7-driven), `port output.a..g` (the lamps).
- Every value and every single-bit transition is tried (study 12's exhaustive walk); each output
  must match the font after settling, robust in every rotation and position.

## The function (per segment, which of the 16 values light it; runs of consecutive values)
| segment | off at | runs |
|---|---|---|
| a | 1 4 b d | 5 |
| b | 5 6 b C E F | 3 |
| c | 2 C E F | 3 |
| d | 1 4 7 A F | 5 |
| e | 1 3 4 5 7 9 | 5 |
| f | 1 2 3 7 d | 4 |
| g | 0 1 7 C | 3 |
An analog design (the value as one signal strength, each segment from windows over it) needs a
comparator pair per run: 28 runs, too many. Each segment is OFF at only 4-6 values, so a decoder
of the off-values (one torch line per value) ORed per segment is the other baseline.

## Board
| design | blocks | status |
|---|---|---|
| (none yet) | | the judge is in (explore/tff), a design is next |

## clk-obs plan (2026-10-21)
- Row form: segment = OR over r = (a3,a2) of R_r AND F_r(a1,a0). One COMPARATOR per (segment, row),
  subtract mode: rear = F_r (an OR of a1/a0 literals, a redstone block for F = 1, a small gate for
  a1'a0 / a1 a0' / a1'a0' / xor / xnor), sides = the a3-literal and the a2-literal (out = F AND
  NOT side1 AND NOT side2 = F AND R_r). 28 comparators (every row of every segment is non-empty).
- Segment cell (tested on redsim, tracks/display/cell-a-test.txt, the OFF form with a NOR torch):
  4 comparators around one cell facing in; their sides are the 4 corner cells, shared by
  neighbouring comparators: NW = a3, SW = a2, SE = a3', NE = a2' gives W = row 0, N = row 1,
  S = row 2, E = row 3. In the ON form the centre is the OUTPUT cell itself (the judge's lamp, the
  OR of the 4 comparators: no torch, 2 gt).
- Strength rule (subtract): a side must be at least as strong as the rear whenever it is on.
- Rows' F (ON form): a [C+D', C+D, C'+D', C+D'], b [1, C xnor D, C'+D', C'D], c [C'+D, 1, 1, C'D],
  d [C+D', C xor D, C'+D, C'+D'], e [D', CD', C+D', 1], f [C'D', C'+D', 1, C+D'],
  g [C, C'+D', 1, C+D] (C = a1, D = a0).
