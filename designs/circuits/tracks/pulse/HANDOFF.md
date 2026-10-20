# Track pulse HANDOFF (contraption study 1: pulse circuits)

Branch `explore/clk-obs` (shared with the clock track clk-obs), crate `crates/circuit-pulse`
(`mctas circuit pulse <cmd>`), designs in this folder, archive in `designs/circuits/pulse/archive/`
(the judge: `mctas circuit sim|verify|archive`, docs/circuits.md). Every mechanism with its
numbers: [NOTES.md](NOTES.md). Board: Space memory `minecraft-tas/circuits/pulse.md`.

## Results (2026-10-20, ~140 designs archived, all judged in the real game)
- Every kind has a long-L extendable card: `pulse-limiter-u4` L = 16 + 16n (subtract comparator +
  repeater U), the bulb one-shots (a pushed observer lights a copper bulb, a repeater U turns it
  off: a bulb toggles only on rising edges) `pulse-oneshot-rise` / `-ext` / `-fall` L = 18 + 16n,
  the bulb-triggered `pulse-oneshot-bulb-rise` / `-ext` (min-pulse 1), `pulse-dual-edge-reset`
  L = 30 + 16n (locked one-shot with an edge reset), and odd: `pulse-limiter-psub-u4` L = 13 + 16n
  (n 0..10), `pulse-rise-psub-u4` and `pulse-fall-psub-u4` (n 0..2: their min-pulse grows with L).
- Every even L 2..40 for rising / falling / limiter / extender (tunable groups), dual-edge 2..8,
  18, 34 and the card.
- Odd L: falling-edge L1 (QC re-extension), dual-edge L3 (two observers 1 gt apart), odd 3..9 by
  OVERLAP for dual / rising / limiter / falling, and odd 3..15 by PISTON-SUBTRACT (a piston-moved
  redstone block on a subtract comparator's rear: the piston moves the tick after the input and
  the block lands in the block-entity phase, so the rear's delay is odd).
- Min-pulse 1: rising-edge L2..8 (bulb + comparator + observer), rising-edge / limiter L2 (two
  sticky pistons, a redstone block and a self-re-arming observer).

## Tools (crates/circuit-pulse)
- `screen <designs>`: a redsim screen of every cell around a design with the judge's output
  levels and rules (limiting / extending witnessed by two inputs, inner toggles allowed, outputs
  back at rest after every input); lists every kind each cell satisfies.
- `census --k K --palette ...`: every polycube of K blocks + an input cell; k = 4 full palette:
  1.96 G sims, 2 h on 50 threads (k = 5 would take ~14 h: not run).
- `family <name> --n N --out F [--card-max-n K]` (cards), `family list`, `limiter-for`,
  `oneshot-for`, `oneshot-mode`, `odd --mode rise|fall|piston --kind K --out-dir D`.
- `trace <design> --w W`: one input pulse tick by tick on redsim (every block change, output).
- `unarchive <files> [--families F]`, `stamp-group --group G [--title T] [--note N] <files>`.
- The catalog folds (owned by this track since 2026-10-20, crates/circuit/src/archive.rs): tunable
  groups (`group:` headers or the tunable key: type key with repeater delays and comparator modes
  masked), equivalent layouts, `catalog.tsv` group column; per-study card counts before -> after.

## Judge notes (found here, fixed by clockinfra)
- A shortening delay line passed as a limiter and a +4 stretcher as an extender: fixed (two
  witnesses each). A design whose output stayed on after a 1 gt input passed: fixed (every gap must
  end with the output off); my piston-subtract designs were reworked with an observer hold.
- New rule (main, 2026-10-20): an output that changes without a block update (redirected dust)
  fails; the one such design (dual-edge 1/2 dust) left the archive.

## Open
- Odd long rising / falling / dual-edge cards for any n (the psub min-pulse grows with L; a
  one-shot whose loop holds a double piston stage would keep it short).
- Odd extenders; a min-pulse 1 falling-edge detector (best: 2).
