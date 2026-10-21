# Adders (WR board)

Kind `adder N` (a compute op): ports `input.a0.., input.b0.., input.cin` (optional) and
`output.s0.., output.cout` (optional), R7-driven inputs. Every input combination is tried in every
transition order (study 12's exhaustive walk); the outputs must settle to the right sum and carry,
robust in every rotation and position. The archive axis (`circuit archive`): "half adder" (carry out
only), "full adder" (carry in and out), "4-bit adder slice" (a `slice:` design: copies chained carry
to carry, judged at 1..4 and 8 bits). Metric: the coordinator ranks by VOLUME (the judge's box).

## Board
| what | design | blocks | box (volume) | latency | status |
|---|---|---|---|---|---|
| half adder | [ha-17b](archive/ha-17b.txt) | 17 | 4x2x3 (24) | 12 gt | robust, SAT-synthesized |
| full adder | [fa3d-36b](archive/fa3d-36b.txt) | 36 | 4x4x4 (64) | 18 gt | robust: two half adders stacked |
| 4-bit adder slice | [fa-slice-54b](archive/fa-slice-54b.txt) | 54 per bit | 8x2x7 per bit (112) | 18 + 12 (N-1) gt | robust at 1..4 and 8 bits |

## Bounds (w86, 2026-10-21; details in [tracks/w86/NOTES.md](tracks/w86/NOTES.md))
- Half adder: none in a 3x2 or 3x3 flat grid, nor in 4x2 or 5x2 (SAT, `circuit arith synth --r7`,
  UNSAT); none in a 3x2 footprint at height 3 (3D model with dust steps). 3x3x3 exists (19 blocks,
  volume 27, worse than 24).
- Full adder below volume 64: every flat half adder found (17-19 blocks) sends its sum out across
  its short side, so the block that carries it to the second half adder adds a row: 4x3 per half
  adder + that row = 4x4 per layer pair, two pairs stacked = 4x4x4. 4x4x3 with the bottom half
  adder fixed is UNSAT (43 s); 3x3 flat half adders (which would leave room) are UNSAT. From-scratch
  full adder searches (flat up to 7x5; 3D 4x3x4, 4x4x3, 3x4x4, 3x3x4) find no model in hours: the
  solver, not a proof, is the wall there. Annealing (`circuit compact`, 60k moves) and the local
  census (remove 2, 940k candidates) find nothing smaller than fa3d-36b.
- Open: a full adder that is not two half adders (one analog stage for the count) could beat 64.
