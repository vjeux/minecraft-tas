# Handoff: tracks delay + gates (studies 3 and 4)

Session 7c07d5cb (formerly clk-rt), box devvm55879 (166 cores, MCTAS_MAX_WORKERS=8), clone
`~/agents/delay/minecraft-tas`, branch `explore/delay` (both tracks). Notes: [delay
NOTES](NOTES.md), [gates NOTES](../../../gates/tracks/gates/NOTES.md).

## State (everything pushed, archived robust in the real game)

* Delay lines (`designs/circuits/delay/archive/`): N=1 in 6 blocks (`delay-piston-qc-1`), N=3 in 9
  blocks (`delay-piston-or-3`, the odd stage); cards `delay-rep2/4/6/8` (group "repeater line", N =
  2 x sum of delays, 2 + 2n blocks), `delay-cmp` (N = 2 + 2n), `delay-piston-or5/7/9/11` (N = base
  + 8n, 13 + 2n blocks), `delay-torch` (N = 6 + 4n). Every N >= 1 has a design.
* Logic gates (`designs/circuits/gates/archive/`): OR (lever 1b, dust 2b, 0 gt), NOT / NOR (lever
  2b; torch 3b; bump 4b at 0 gt), NIMPLY(-BA) (comparator 2b; bump 4b at 0 gt), IMPLY(-BA) (torch 3b;
  bump 4b at 0 gt), NAND (torch 6b; bump 8b at 0 gt), AND (comparators 5b, 4 gt). XOR and XNOR open.
* The judge: "a circuit must drive something" (crates/circuit `check::update_less`,
  `Circuit::{consumer_cells, with_consumers, watched_lamps, passive_output}`, docs/circuits.md),
  `circuit archive --rejudge`, selftests in crates/piston-door/src/selftest.rs (circuit:sim- and
  verify-calibration).

## Commands

* `mctas circuit delay family <rep2|rep4|rep6|rep8|cmp|or5|or7|or9|or11|torch> --n N --out F`;
  `family-check` (every member on redsim vs the formulas); `piston-or --n N`; `prune <files>`
  (drop covered instances from an archive); `census` (exhaustive, `--fixed` seed blocks).
* `mctas circuit gates census --dims 3x2x3 --inputs "a;b|..." --palette ... --k 5` (hits confirmed
  and minimality decided by the judge's redsim screen, lamp included).
* Never rebuild `target/release/mctas` while a judge run that spawns generators is going (family
  members are built by running the current executable: after a rebuild its path is gone and every
  family fails "No such file or directory"); run long judges from a copy (`cp target/release/mctas
  /tmp/mctas-x`).

## Open

* N = 3 and odd N >= 5 smaller than 9 / 13 blocks: the N=1 tricks (input above a dust, a
  quasi-connected piston, the power bump) may shrink the OR stage.
* XOR / XNOR update-correct: one dust cannot (two transitions need a power change); two subtract
  comparators with both inputs direct face opposite ways (their outputs are 3 cells apart), so the
  smallest is probably 7-8 blocks; XNOR = 15 - max(a - b, b - a) on a third comparator.
* Smaller odd tails than 13 blocks (the OR before the tail costs a dust and its stone).
