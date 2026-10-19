# clk-rt extendable families (for the catalog's `extend:` cards)

Every family below grows by repeating one unit; `mctas clock rt ...` builds any member. "Verified"
= instances archived with `clock archive` (robust, 32 builds). Delays d are repeater delays (1-4);
the fixed-delay series uses d = 4 everywhere (other delays fill the periods in between, see the
general rule).

| family | base | unit added | P | blocks | verified | generator |
|---|---|---|---|---|---|---|
| 2-torch U-ring (redstone_torch+repeater) | T-4*-4*-T, P20, 8b | one delay-4 repeater in each row | 20 + 16n | 8 + 4n | n = 0, 1 exactly; with tuned delays every even P 10..98 (k = 1..6) | `uloop --head torch --top 4*,4*,.. --bottom 4*,4*,.. --tail torch` |
| 1-torch inverter U-loop (redstone_torch+redstone_wire+repeater) | C-4*-4*-T, P36, 9b | one delay-4 repeater in each row | 36 + 32n | 9 + 4n | n = 0..2 (every P = 0 mod 4 from 12 to 100) | `uloop --head corner --top 4*,.. --bottom 4*,.. --tail torch` |
| repeater pulse ring, 3 x h rectangle (repeater) | 3x3: b,4*,b,4,b,4,b,4, P32, 12b | two delay-4 repeaters (h + 1) | 32 + 16n | 12 + 4n | n = 0..4 (P8..P100 even, tuned) | `ring --w 3 --h H --cells ...`, `even-rect --kind pulse --from P --to P` |
| 1-torch inverter, 3 x h rectangle (redstone_torch+redstone_wire+repeater) | 3x3: b,4,b,4,b,T,d,4, P52, 12b | two delay-4 repeaters (h + 1) | 52 + 32n | 12 + 4n | n = 0..1 (P40..P52, P72..P84) | `even-rect --kind inv --from P --to P` |
| odd two-pulse ring, 3 x h rectangle (repeater) | 3x4: b,4*,b,4,4,b,4,b,4,3, P23, 16b + kick | two delay-4 repeaters (h + 1) | 23 + 8n (odd) | 16 + 4n | n = 0..9 (every odd P 5..99) | `odd-rect --from P --to P` |
| comparator fading ring (comparator+redstone_torch+redstone_wire) | D-c,c-c,c-F, P114, 18b | one comparator in each row | 114 + 56n | 18 + 4n | n = 0..6, 8, 10 (P114..P450, P562, P674) | `uloop --head dust2 --top c,c,.. --bottom c,c,.. --tail fade` |
| divider chain (redstone_torch+redstone_wire+repeater) | U-loop 4(1 + d1 + d2) | one T flip-flop stage (18 blocks) | P0 x 2^n | B0 + 18n | P40..P2304 (base 9b) | `divider --d1 D1 --d2 D2 --stages N` |
| divider on a fading ring (+comparator) | fading ring 56c + 2 | one T flip-flop stage | (56c + 2) x 2^n | 4c + 10 + 18n | P228..P10816 | `divider --fade C --stages N` |
| comparator counters (comparator+redstone_torch+redstone_wire) | fading ring P114, 18b | one counter level (2 + 2 comparators + shaper, 26 blocks) | 114 x 14^n | 18 + 26n | n = 1, 2, 3 (P1596, P22344, P312816) | `counter --k-in 2 --ks 2[,2,2..]` |

General rules (any delays, not only 4):
- 2-torch U-ring with k repeaters per row: P = 4 + 2 x (sum of delays), 4k + 4 blocks.
- 1-torch inverter U-loop: P = 4 x (1 + sum of delays), 4k + 5 blocks.
- Rectangle pulse ring: P = 2 x (sum of delays) (one loaded pulse); odd ring: P = sum of delays (two
  pulses, every delay at most (P - 1) / 4); 4 corner blocks + 2 per repeater (+1 relay block when
  the repeater count is odd and a side has 3 straight cells, else + dust).
- Fading ring with k comparators per row: P = 56k + 2 (k >= 2 robust; k = 1 is P58/P60 by build).
- Counter level: x14 when its lap (4 gt per comparator pair) matches the inner pulse within 1 gt
  (fading k = counter k), or 10 gt pulses into a lap of 8 (counter over counter, k = 2).
