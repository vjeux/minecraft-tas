# clk-obs9: the thermometer decoder after the face rule (2026-10-21)

`mctas circuit display therm --out F` (crates/circuit-display/src/therm.rs), then
`display equalize F --at=-7,0,78` (the chain's output dust) and `display settle F --frames 600`.

| design | blocks | box | latency | glitches | game |
|---|---|---|---|---|---|
| therm-v1 | 1989 | 41x8x80 (26,240) | 178 gt | 28 | robust |
| **therm-v2** (equalized) | 1989 | 41x8x80 (26,240) | 178 gt | **0** | robust |

How it works:
- The analog chain (code w [4,1,8,2], every bit complemented) makes s; its four inputs come in
  from the back row.
- Per segment a STRIP: a dust ruler from s (cell j carries s - j); per run [lo, hi] of the
  segment's ON strengths one subtract comparator C: rear a delay-2 tap repeater on the ruler cell
  lo-1 (T_lo), side the tap on cell hi (T_hi+1, a delay-1 tap + a delay-1 repeater into C's side,
  4 gt either way). A run touching s = 0 takes a redstone block as rear, a run touching 15 no side.
  Every tap reads the same dust, so a window's two inputs switch in the same tick: glitch-free once
  s itself moves in one step (equalize).
- A feeder that ends next to the following window's comparator ends in a dust (feeds its side):
  runs alternate between the two sides of the ruler.
- Each side's comparators power an OR row (blocks, dust on top); one repeater per row, both
  returned as equal dust into one block X; the strip's output repeater reads X.
- Strips in four depth bands beside a lattice of repeater lanes (one per lamp, the checkerboard
  cells behind the 3x5 face, stone in the others); lamps at z = 0, outside the box.
- Two lossless spines (comparator/block, height steps with dust over/under a block) carry s to
  every strip's hub. A height step's dust must stay 2 cells away from a band (it joined a strip's
  return row: wrong outputs).

Volume next (ideas, not built):
- One band: strips stacked 4 apart on each side, stairs to the lamp rows (~9,000).
- Two segments per ruler (one side each): a feeder end next to the next comparator must end in a
  BLOCK (a comparator's side ignores blocks); single-value runs still need a dust there.
- Dense: T_t as blocks K_t (tap repeater into a block); a window is one comparator with K_lo as
  rear and a dust standing on K_hi+1 as side (both switch with K: glitch-free); the face allows
  ~17 direct comparator feeds (behind each lamp, the face plane's even cells, the outer cells beside
  f/e/b/c), the rest through relay comparators from OR blocks.
