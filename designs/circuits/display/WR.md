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
| **seven-segment-analog** (archive, tracks/display/analog-v5.txt) | **566** | **7x16x7 (784)** | **record**: game robust, latency 34 gt, 65 glitches (the worst build; the judge reports glitches as the max over builds since 2026-10-21) |

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

## The display face (vjeux 2026-10-21 11:43Z: "the output is in the middle of the redstone contraption, how do you feed it to the display?")
The judge now requires the seven outputs to be LAMPS forming the standard 3-wide x 5-tall
figure-8 on ONE vertical exterior face, read from outside (`compute::FACE`: a (1,0), f (0,1),
b (2,1), g (1,2), e (0,3), c (2,3), d (1,4), column and row from the top; f on the viewer's left),
with no design block and no judge input device in front of it. Every design above put its lamps
inside the stack and was rejudged out (designs/circuits/display/tracks/rejudged/). The board
restarts: minimum volume with the face fixed, glitches reported (the worst build's).

| design (display-face judge) | blocks | box (volume) | status |
|---|---|---|---|
| seven-segment-therm-v2 (tracks/display) | 1989 | 41x8x80 (26,240) | game robust, latency 178 gt, glitch-free |
| seven-segment-therm-v3 (tracks/display) | 1793 | 47x7x52 (17,108) | game robust, latency 130 gt, glitch-free |
| seven-segment-therm-v4 (tracks/display) | 1465 | 47x7x39 (12,831) | game robust, latency 108 gt, glitch-free |
| seven-segment-therm-v5 (tracks/display, clk-obs10: the tower) | 1441 | 19x13x28 (6,916) | game robust, 0 glitches; A*-routed lanes |
| seven-segment-therm-v7 (tracks/display) | 1411 | 18x13x24 (5,616) | game robust, 0 glitches (lane zone 5 deep) |
| seven-segment-therm-v9 (tracks/display) | 1176 | 18x10x23 (4,140) | game robust, 0 glitches: shared rulers (a/b/d/g two per ruler) |
| seven-segment-therm-v10 (tracks/display) | 1033 | 18x7x29 (3,654) | game robust, 0 glitches: 4 shared rulers (s = 0 windows fed from the hub), chain behind the strips |
| seven-segment-therm-v11 (tracks/display) | 1062 | 18x7x27 (3,402) | game robust, 0 glitches: the chain tucked behind the shortest strip |
| **seven-segment-therm-v12** (archive) | **1046** | **18x7x27 (3,402)** | **glitch-free record**: game robust, latency 84 gt, 0 glitches (this layout's floor: ~1,600 more bases searched; next: the chain in a 5-wide gap, est. 3,060) |
| seven-segment-rings (clk-item; tracks/display/clk-item/rings-v1.txt) | 849 | 23x7x21 (3,381) | (2026-10-21) game robust, latency 78 gt, 55 glitches. clk-obs8's plan 1 (`display rings --w 4,1,8,2 --copies 15`) made to work: three ring stacks (a/g/d behind the face, f/e and b/c beside it), each ring one level above its lamp, output under the front axis, repeater lanes to the lamps. Three fixes in crates/circuit-display/src/rings.rs: (1) a lower ring's feed now includes what the ring above couples into it (its bridges' and copies' target blocks sit right over the lower ring's arc cells; ignoring it made e wrong at 4 values); (2) e's descent no longer starts west of f_rear, where f's second-feed dust overwrote its first comparator (e was fed only by coupling); (3) no line block under a line dust: d's descent's block sat under g's descent dust, and e's under its own first dust, two lossless loops that latched (d, g stuck after 7 -> 5; e after 5 -> 4) |
| seven-segment-rings2 (tracks/display/clk-item/rings2-v1.txt) | 765 | 23x7x19 (3,059) | game robust, latency 72 gt, 56 glitches: `display rings2 --lean` (crates/circuit-display/src/rings2.rs): rings solved first, wired after; g, d and e need no feed line of their own (the ring above's coupling feeds them); the chain right behind the lines (hub 2 moves from the split) |
| seven-segment-rings2-v2 (tracks) | 699 | 23x7x19 | game robust, 70 gt, 53 glitches: `--prune` leaves out the arc corners and cells a ring does not need (bridge and copy endpoints kept: astack's distances carry a bridge into a removed end) |
| seven-segment-rings3 (tracks/display/clk-item/rings2-v3.txt) | 677 | 23x7x19 (3,059) | game robust, latency 70 gt, 47 glitches: `display rings2 --lean --prune`, the prune also turns off bridges a ring does not need (then their ends may go) |
| seven-segment-rings4 (tracks/display/clk-item/rings2-v4.txt) | 659 | 23x7x19 (3,059) | game robust, latency 70 gt, 29 glitches: the prune may also drop unused copy ends and coupling cells, empty slots lose their stone; then `display equalize --at 2,4,13` (the hub's comparator) slows the faster inputs so every bit reaches the hub at 26 gt (47 -> 29 glitches). Rings 63/66/75/63/63/69/45 blocks (a g d f e b c), chain ~55, lines ~45, lanes ~80 |
| seven-segment-rings5 (tracks/display/clk-item/rings2-v5.txt) | 651 | 22x7x19 (2,926) | game robust, latency 70 gt, 29 glitches: `display rings2 --lean --prune --tight` then `display equalize --at 2,4,13`: the f/e stack one column closer (cx 8; its line feeds at ring cells (-2, 0, 3) / (-1, 0, 3) so its line is one move shorter; its west bridge forced off, since that bridge's target block would sit beside ring g's arc at g's level; a stone above any lower dust a dust step from the other stack would reach: 3 cuts). The b/c stack cannot come closer: with its east bridge off ring b has no solution |
| seven-segment-rings6 (tracks/display/clk-item/rings2-v6.txt) | 651 | 22x7x19 (2,926) | game robust, latency 74 gt, 4 glitches (was 29): `--deglitch 3`, each lane's first repeater at delay 3. Java's DiodeBlock re-checks its input at the scheduled tick, so a repeater stays on through an input dip shorter than its delay: every hand-over dip (4 gt) of the rings is gone; then `display equalize --at 2,4,13` and `display settle` (the slower lanes would not have settled by the judge's rest check). Left: two 6 gt pulses (g on 0001 -> 0000, d on 1111 -> 0111), which a repeater extends instead (a NOT - repeater - NOT stage would remove them) |
| **seven-segment-rings7** (archive; tracks/display/clk-item/rings2-v7.txt) | **643** | **22x7x19 (2,926)** | **record**: game robust, latency 74 gt, **2 glitches**: `--off d`: ring d computes NOT d (its OFF runs; 8 blocks cheaper), and its lane is the delay-3 repeater, the stone, a wall torch (the inversion) and a delay-4 repeater into the lamp, same 4 cells: the first repeater drops NOT d's short dips (d's pulses), the second d's short dips. Left: g's 6 gt pulse on 0001 -> 0000 (g in OFF form needs its own feed line, whose descent hits b's line) |

therm-v2 (designs/circuits/tracks/display/FACE-NOTES.md, plan 3): the analog chain gives s; a dust
ruler gives every threshold; each run is one subtract comparator (rear = a delay-2 repeater tap at
lo, side = the tap at hi + 1); repeater lanes to the face lamps; the inputs' delays equalised
(`display equalize --at`) so every change reaches the windows in the same tick. Next: volume.
