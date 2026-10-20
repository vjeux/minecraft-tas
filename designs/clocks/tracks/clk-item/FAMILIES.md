# clk-item: extendable item-clock families (cards)

Each card is a base design with `extend` headers in `designs/clocks/tracks/clk-item/cards/`
(written by `mctas clock item cards --write designs/clocks/tracks/clk-item/cards`; the card table is
`crates/clock-item/src/extend.rs::cards`). `mctas clock item cards --check` runs every member of the
redsim cards on redsim (all match over the whole n range); `clock archive <card>` judged 5 members
of each in the game (low, low+1, low+2, middle, high). Generator: `mctas clock item gen <family>
... --n {n} --out {out}`.

| card | P(n) gt | blocks(n) | n | unit |
|---|---|---|---|---|
| clk-item-hopper-ring | 14 + 14n | 4 + 2n | 0..40 | one column (2 hoppers); n = 0 is the hopper pair |
| clk-item-hopper-ring-2 | 7 + 7n | 4 + 2n | 1..40 | same ring, two items half a ring apart |
| clk-item-ringlock-rows | 76 + 14n | 12 + 2n | 0..30 | one row of the 3-wide ring lock (5 stones) |
| clk-item-ringlock-stones-3x6 | 104 + 8n | 16 | 0..4 | one more stone (5 + n) in the 3x6 ring lock |
| clk-item-ringlock-stones-3x20 | 300 + 8n | 44 | 0..28 | one more stone (5 + n) in the 3x20 ring lock |
| clk-item-lockpair | 11 + 2n | 9 | 0..2 | lock repeater delay n + 1 (pair, 2 swords) |
| clk-item-lockring-2s | 33 + 2n | 11 | 0..3 | lock repeater delay n + 1 (2x2 ring, 2 swords): 33-39 |
| clk-item-lockring-3s | 41 + 2n | 11 | 0..3 | lock repeater delay n + 1 (2x2 ring, 3 swords): 41-47 |
| clk-item-snowdisp | 16 + 4n | 5 | 0..2 | repeater delay n + 1 (JVM only, kicked) |
| clk-item-dropring-2d | 34 + 14n | 12 + 2n | 0..30 | one column of the dropper ring (W = 4 + n), two droppers |
| clk-item-dropring-1d | 27 + 14n | 10 + 2n | 0..30 | W = 3 + n, one dropper |
| clk-item-dropring-2d-r1 | 43 + 14n | 16 + 2n | 0..30 | W = 4 + n, repeater delay 1 |
| clk-item-dropring-2d-r2/r3/r4 | 31/33/35 + 14n | 14 + 2n | 0..30 | W = 3 + n, repeater delay 2/3/4 |
| clk-item-dropring-1d-r1..r4 | 36/38/40/42 + 14n | 14 + 2n | 0..30 | W = 3 + n, one dropper, delay 1..4 |
| clk-item-dropring-2d-r14/r24 | 44/46 + 14n | 18 + 2n | 0..30 | W = 3 + n, repeaters 1+4 / 2+4 |
| clk-item-dropring-1d-r14/r24 | 51/53 + 14n | 18 + 2n | 0..30 | W = 3 + n, one dropper, 1+4 / 2+4 |

The fourteen dropper-ring cards are one per residue of P mod 14: together they reach every P from
42 to about 460 (and 27, 31, 33-36, 38, 40 below); the formula holds for every width checked
(W <= 34 on redsim). The general formula of the ring (any width W, depth 3 + repeaters, one or two
droppers): P = 20 + 14 (W - 3) + 7 (depth - 3) + 7 [one dropper] + 2 x (sum of delays), blocks
2W + 4 depth - 8.

Ring lock in general (3 x d, L = 2d + 2, N stones in the corner): P = 7L + 8N - 34 for
5 <= N <= floor((7L - 2) / 8) - 3 (257/257 members up to d = 20 match on redsim).

Not linear (kept as individual clocks): the jukebox carousel (P = sum over its 3 discs of
20 len + 26: 1446, 6926, 17978 archived), the self-locking pair (15, 17, 20), the burnout hybrids,
the ring locks with sword placements, the digital subtract-comparator versions (8, 16, 24).
