# clockinfra HANDOFF (the clock study's scaffolding and judge)

Owner of `crates/clock*` (except the explorers' own stubs `crates/clock-{obs,rt,sculk,item}`),
`docs/clocks.md` (the one home: definition, judge, format, commands, census results) and
`crates/redsim`. Branch `explore/clockinfra`; everything below is merged or pushed there.

## What exists
- `crates/clock` (optimized library): `Clock` (door design format + `port output` + `port start:
  x y z (last | use [T] | <state> <T>) [at N]` + `placement: player` + `nbt:` lines),
  `sim` (exact redsim detector: `World::state_key` first repeat, then the clock's period = smallest
  divisor over which every cell's full state repeats; output = port or longest own period;
  COMPOSITE = output faster than the whole AND independent spatial groups; families = in-place
  changes + moving redstone blocks), `verify` (the real-game judge: positions x 4 rotations, block
  states + block entities every tick, >= 3 periods, no entities, edge check, robust /
  position-dependent / rotation-dependent, cyclic pulse comparison, output fixed once through the
  rotations), `wave` (activity, family names, waveforms, tail periods, spatial groups), `canon`
  (state rotation/mirror on text, type keys, chirality), `catalog` (archive + CATALOG.md + coverage
  + gallery + composite section), `census` (exhaustive, resumable, minimal flags), `shot` (real
  client pictures), `video` (coordinator clips).
- `crates/clock-cli` (unoptimized): `mctas clock sim | verify | archive | catalog | census | shot |
  video | obs | rt | sculk | item`.
- `crates/redsim/src/snapshot.rs`: exact state keys (+ `key_into` helpers in piston/moving.rs,
  sculk.rs, redstone/container.rs, gravity.rs).
- Calibration: `designs/clocks/calibration/` (8 clocks, all robust in the game, redsim agrees);
  `door selftest` checks `clock:*` (sim-calibration, sim-negative, verify-calibration quick,
  verify-negative full).
- Census results: `designs/clocks/census/<box>-k<k>/` (summary.md, minimal.tsv, designs/).

## Decisions taken (all in docs/clocks.md)
- Kick at tick 0 between ticks (or `at N`); self-start = the door judge's placement.
- Period on the full cell state (block + comparator strength + container contents); pending-tick
  order and torch memory only in the exact whole-state key (`state_period`, printed apart).
- Composite (coordinator's ruling + refinement accepted 2026-10-19 16:18Z).
- Family: in-place changes only (clk-obs's moved-lamp inflation); comparator strengths and
  container contents count.
- Box = swept box; ranking blocks, then swept volume.

## Open / next
- Census: re-run the boxes with the final rules (in-place family, composite) and the larger ones
  (3x3x2 k4 ~ 30 min with 160 threads; 3x3x3 k3); the item palette (hoppers with nbt) is clk-item's
  census.
- `clock video` gallery/montage per period (coordinator records); `clock shot --strip` for every
  archived clock (motion strips) if the catalog wants them.
- Impossibility table (`designs/clocks/impossible.tsv`): fill from census proofs (e.g. no clock
  with <= 2 blocks other than the observer pair; per-family minimum periods).
