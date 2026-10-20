# Study 12, computational redstone: HANDOFF (PAUSED)

Paused on 2026-10-20 at 12:46Z, by the coordinator after vjeux's "world record kind of implementations
for every single one ... quality over quantity". clk-obs moved to world-record quality on studies 1
and 10. Whoever resumes this study should hold every design to that bar: per variation, the record
target derived from mechanics (smallest, flat, tileable per bit, instant or fastest), met or proven
impossible. No first implementations.

## Landed (branch explore/compute)
- [VARIATIONS.md](VARIATIONS.md) (9c95b4204): kinds, encodings, component delays from source, judge
  method, variation table.
- The judge (541b6b75b, crates/circuit/src/compute.rs; docs/circuits.md, "Study 12"):
  - kinds `adder|subtractor|incrementer|comparator|mux|demux|decoder|encoder [priority]|binary-counter N`;
  - exhaustive truth tables: Gray order, then the n-cube Euler tour (n <= 10), then complement jumps;
  - metrics: latency with the worst transition, glitches, per bit;
  - bit slices (`slice:` + chain ports) stacked to 2..4 copies, metric `slice` = blocks per bit + the
    latency law.
- Hex (dec57f1e0): `... N hex`. Values are signal strengths: a design chest the judge fills; the
  value outputs must equal the level.
- Calibrations (all robust in the game; in the full selftest):

  | design | kind | result |
  |---|---|---|
  | `cal-dust-encoder2` | encoder 2 | instant, 16 blocks |
  | `cal-bulb-counter2` | binary-counter 2 (waxed copper bulbs) | 6 gt, 8 blocks |
  | `cal-bulb-counter-slice` | its 1-bit slice | 5 blocks per bit, latency 2 + 4 (N-1) gt |
  | `cal-hex-gt` | comparator 4 hex | 4 blocks, 4 gt |
- Refusal: `cal-dust-encoder2-claims-priority` (a plain encoder claiming priority).

## The split
- pext (session ec7bcf52): adders, subtractors, incrementers, comparators, including hex and
  bit slices. Their tool is crates/circuit-arith (`mctas circuit arith check|synth`): a SAT
  synthesizer of flat designs, CEGAR-checked on redsim, which minimizes blocks.
- clk-obs: muxes, demuxes, decoders, encoders, binary counters.

## Work in progress: `mctas circuit compute census` (crates/circuit-compute)
- What it does:
  - fills a box around fixed input cells with every placement of kmin..=k palette blocks;
  - screens each filling with the judge's own stimulus and ideal outputs, every candidate output
    cell (box + ring) read at once;
  - gives each output a distinct matching cell; writes the smallest and the fastest design.
- Options: `--fixed` (blocks every filling keeps) and `--floor` (the y = 0 palette, e.g. stone).
- Speed: a k = 5 run in a 4x2x3 box with a 7-state palette takes about 45 s.
- Results so far (all `mux 1`):
  - Nothing with k <= 5 in a 3x2x3 box (palette stone, dust, torch, rep, cmp; s0 north, d0 west,
    d1 east).
  - Nothing with k <= 5 extra blocks around two fixed facing subtract comparators (palette stone,
    dust, torch). The reason: a comparator's side reads only dust, diodes and redstone blocks
    (`getControlInputSignal`), and a torch's direct signal goes only up, so NOT s needs a dust or a
    diode.
  - The 3-comparator plan, about 7 blocks:
    - d0 AND NOT s: a subtract comparator, d0 at its back, s at its side;
    - NOT s: a constant redstone block R, minus s, the gates study's AND trick;
    - d1 AND s: d1 minus NOT s.

    Nothing found with comparator 0 fixed (s0 north of it) and d1 at (3,1,1) or (2,1,3), searching
    rblock and cmp on y = 1 and stone floors, k <= 5. The constraint: s must touch the side of both
    comparator 0 and the NOT comparator. Next: route s with one dust, or move d1.
- Next designs:
  - decoder 2: two NIMPLY comparators share diagonal inputs (each reads one input at its back and
    the other at its side), plus a NOR (block + torch) and an AND (a - (a AND NOT b) = a AND b, a
    comparator reading the NIMPLY output);
  - demux 1 = NIMPLY + AND;
  - priority encoder 2;
  - binary counters: a synchronous one, up/down and reset variants; counter min-period is not
    measured yet.
- Open judge question: a plain encoder is pure wiring (OR of inputs), and adjacency alone can
  make a 0-block "design". Decide whether compute kinds need at least one block, or which variations
  count.
