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

## Board (clk-obs, 2026-10-21)
| design | blocks | box (volume) | status |
|---|---|---|---|
| seven-segment-towers (tracks/display/towers-v1.txt) | 2118 | 27x42x28 (31,752) | game robust: the first working build |
| stack-v1 | 982 | 9x38x9 (3,078) | game robust |
| stack-v2 | 697 | 9x26x9 (2,106) | game: every build passes (one rotation counts another glitch total) |
| seven-segment-stack-v3 (tracks/display/stack-v3.txt) | 649 | 9x24x9 (1,944) | game, latency 38 gt, 76 glitches (left the archive: v4 is smaller) |
| seven-segment-stack-v4 (archive) | 627 | 9x18x9 (1,458) | game robust, latency 54 gt, 65 glitches |
| seven-segment-stack-v5 (archive) | 655 | 9x18x9 (1,458) | game robust, latency 56 gt, **62 glitches** (the low-glitch variant) |

| analog-v2 (tracks/display) | 632 | 9x16x9 (1,296) | game robust, latency 86 gt, 76 glitches: the analog ROM, a riser |
| analog-v3 (tracks/display) | 565 | 7x16x9 (1,008) | game, 48 glitches (one build 46): no riser, s flows down the rings' bridges |
| analog-v4 (tracks/display) | 565 | 7x16x8 (896) | game, 64 glitches |
| **seven-segment-analog** (archive, tracks/display/analog-v5.txt) | **566** | **7x16x7 (784)** | **record**: game, latency 34 gt, 64 glitches ("position-dependent": one build counts 63; every build passes) |

The analog ROM (clk-obs8, designs/circuits/tracks/display/ANALOG.md): a subtract-comparator chain
on top turns the bits into one strength s = 15 - (8 a0' + 2 a1 + 4 a2 + a3) (the code searched over
384 bit orders and complements for the fewest windows); s flows down losslessly through comparator
bridges; seven rings every 2 levels, each lamp lit by window comparators (a hopper rear sets the
window; the sides read s and an offset copy of its complement).

The ring stack's floor is 9x9x14 = 1134 (a 7x7 footprint, two rings per level and a first ring
at level 2 were all searched or built and fail; clk-obs7's report). The next structure is the
analog ROM (clk-obs8): the bits as one signal strength, each segment from windows on it.

How v3 works: seven rings of 4 subtract comparators stacked every 2 levels in one column; ring k's
lamp is segment k. The 4 ring corners come from 4 torch towers in a pinwheel; each rear column is a
torch chain (rear = NOT the rear two levels down) plus an outer injection at (0,-3) and a diagonal
one at (-1,-2) (a repeater, or a subtract comparator whose side is fed by a repeater); all towers
start from the 4 inputs through wall torches (a wall torch on a tower block starts the next column a
level up). Generator: `mctas circuit display stack-search` then `display stack --forest
--rings-first` (crates/circuit-display). Open: the 9x9 form's floor is 9x9x13 = 1053 even with no
feed; a 7x7 footprint has no solution with these injection faces (the a1/a0 XOR rows of b and d need
two product terms per rear).
