# Adders (study 12, kind `adder N`): track w86

Coordinator 2026-10-21 15:23Z: the 1-bit full adder (a0, b0, cin -> s0, cout) at minimum volume, then a
4-bit ripple adder built from it as a bit slice. Judge: `circuit sim` (redsim) and `circuit verify` (game),
R7 inputs (the judge's repeater drives each input cell). Study page: designs/circuits/compute/VARIATIONS.md.

## Designs (game-judged with `circuit verify`)

| design | kind | blocks | box | latency | glitches | how |
|---|---|---|---|---|---|---|
| [ha-17b](ha-17b.txt) | adder 1 (half adder) | 17 | 4x2x3 | 12 gt | 4 | synthesized (`circuit arith synth --r7`, 5x3 grid) |
| [fa-dumb-57b](fa-dumb-57b.txt) | adder 1 (full adder) | 57 | 9x2x7 | 24 gt | 16 | two ha-17b, the second fed by the first's sum stone; carries OR'ed by a dust line |
| [fa-43b](fa-43b.txt) | adder 1 (full adder) | 43 | 6x2x7 | 18 gt | 11 | two ha-17b: the first's sum stone is the second's b input, cin its a input (torch); the first carry reaches the output lamp by a repeater + 3 dust, the second carry's stone touches the lamp |

## Tools

* `circuit arith synth --r7` (2026-10-21): the arith track's SAT synthesizer with R7 input cells (the
  judge's driven stone: dust never points at it, no comparator side input, torches may hang on it) and
  every model judged by the circuit judge's own redsim run. `--sym a0,b0,cin` breaks the input symmetry.
  `--chain cin,cout` with fixed cells `--in cin=x,y,z --out cout=x,y,z` (the others still auto): a bit
  slice, the chain output a redstone lamp of the design judged on its own direct power, written with
  `slice:` (step = cout - cin) and judged as a slice (N = 1..4, 8). First result: an incrementer slice
  (40 blocks per bit, 6x2x4, latency 10 + 6 (N-1) gt) in a 5x4 grid in 90 s.

## Log

* 15:47Z half adders from the synth: 17 blocks (5x3 grid), 19 (4x3), 22 (4x4); ha-17b robust in the game.
* 16:05Z fa-dumb-57b robust in the game (first full adder). The full adder from scratch (grids 4x4..7x5,
  height 2 and 3, with and without symmetry breaking) has no first SAT model after 25 min.
