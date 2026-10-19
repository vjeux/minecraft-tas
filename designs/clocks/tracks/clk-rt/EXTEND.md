# clk-rt extendable families (archived as `extend:` catalog cards)

Each family is ONE base design in [families/](families/) with `extend` headers (docs/clocks.md
"Extendable clocks"); `mctas clock rt family <name> --n N --out F` builds the n-th member and
`--card` adds the headers. Every n of every range below passed `mctas clock extend-check` in the
game (period and blocks equal to the formulas, robust); `clock archive` judged the samples again and
recorded the instances in `designs/clocks/archive/extend.tsv`. The individual archived clocks that
were exactly instances (38) were removed from the archive (`mctas clock rt prune-members --apply`).

| card | P (gt) | blocks | n judged | the unit added |
|---|---|---|---|---|
| clk-rt-uloop2t | 20 + 16n | 8 + 4n | 0..10 | one delay-4 repeater in each row of the 2-torch U-ring |
| clk-rt-uloop1t | 36 + 32n | 9 + 4n | 0..8 | one delay-4 repeater in each row of the 1-torch inverter U-loop |
| clk-rt-rect-pulse | 32 + 16n | 12 + 4n | 0..10 | the 3 x (3+n) repeater ring one row longer (corner blocks, one loaded pulse) |
| clk-rt-rect-inv | 52 + 32n | 12 + 4n | 0..8 | the 3 x (3+n) inverter ring (one torch on a corner block) one row longer |
| clk-rt-odd-rect | 23 + 8n (odd) | 16 + 4n | 0..9 | the 3 x (4+n) two-pulse ring one row longer (odd-tick kick) |
| clk-rt-fade | 114 + 56n | 18 + 4n | 0..10 | one comparator in each row of the comparator fading ring |
| clk-rt-div | 36 x 2^n | 9 + 18n | 1..7 | one master-slave T flip-flop stage on the P36 loop |
| clk-rt-div-fade | 114 x 2^n | 18 + 18n | 1..5 | one T flip-flop stage on the P114 fading ring |
| clk-rt-counter | 114 x 14^n | 18 + 26n | 1..3 | one comparator counter level (x14) around the P114 fading ring |

General rules (any delays: the cards fix d = 4; other delays give the periods in between, and those
tuned designs stay archived as individual clocks):
- 2-torch U-ring with k repeaters per row: P = 4 + 2 x (sum of delays), 4k + 4 blocks.
- 1-torch inverter U-loop: P = 4 x (1 + sum of delays), 4k + 5 blocks.
- Rectangle pulse ring: P = 2 x (sum of delays) (one loaded pulse); odd ring: P = sum of delays (two
  pulses, every delay at most (P - 1) / 4); 4 corner blocks + 2 per repeater (+1 relay block when
  the repeater count is odd and a side has 3 straight cells, else + dust).
- Fading ring with k comparators per row: P = 56k + 2 (k >= 2 robust; k = 1 is P58/P60 by build).
- Counter level: x14 when its lap (4 gt per comparator pair) matches the inner pulse within 1 gt
  (fading k = counter k), or 10 gt pulses into a lap of 8 (counter over counter, k = 2); a counter
  decrementing 2 or 3 levels per pulse counts 7 or 5 (`counter-over --dec`).
