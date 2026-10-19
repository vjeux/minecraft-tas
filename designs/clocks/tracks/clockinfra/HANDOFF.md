# clockinfra HANDOFF (the clock study's scaffolding and judge)

Owner of `crates/clock*` (except the explorers' own crates `crates/clock-{obs,rt,sculk,item}`),
`docs/clocks.md` (the one home: definition, judge, format, commands, census results) and
`crates/redsim`. Branch `explore/clockinfra`; everything below is pushed there and merged by the
coordinator (f1a42990).

## What exists
- `crates/clock` (optimized library):
  - `Clock`: the door design format + `port output` + `port start: x y z (last | use [T] |
    <state> <T>) [at N]` + `placement: player` + `nbt:` lines.
  - `sim`: exact redsim detector (`World::state_key` first repeat; the clock's period = smallest
    divisor over which every cell's full state repeats: block state + comparator strength +
    container contents); output = port, else the changing cell with the longest own period;
    COMPOSITE = output repeats faster than the whole AND independent spatial groups (coordinator's
    ruling 2026-10-19 + refinement); families = in-place changes + moving redstone blocks.
  - `verify`: the real-game judge, 4 positions x 4 rotations by default (vjeux), horizon = W + 4P
    from redsim (<= 20000 gt), sparse frames (non-air cells) + segmented bridge reads (<= ~400k
    cells per message: big boxes and long horizons fit), cyclic pulse comparison, output fixed
    once through the rotations.
  - `wave`, `canon` (type key over 4 rotations AND mirror images + nbt; `chiral`), `catalog`
    (archive with SAME TYPE refusal per (period, type) + `--dedupe` + aliases.tsv + name
    collisions -> `<name>-<6 hex type hash>`; CATALOG.md = gallery only; coverage.md = matrix),
    `census` (exhaustive, resumable, MINIMAL flags, `--max-period` early pruning, `full` palette,
    `census-report`), `shot` (real-client pictures with the orange-glass output marker;
    per-sandbox (default, reliable) or `--grid` one-world batches; `shot-compare` SSIM check),
    `video` + `montage` (coordinator records).
- `crates/clock-cli` (unoptimized): `mctas clock sim | verify | archive | catalog | census |
  census-report | shot | shot-compare | video | montage | obs | rt | sculk | item`.
- `crates/redsim/src/snapshot.rs`: exact state keys.
- Calibration: `designs/clocks/calibration/` (8 clocks, robust in the game, redsim agrees);
  `door selftest` checks `clock:*` (sim checks quick; verify checks in the full suite).
- Pictures: `designs/clocks/img/<archive stem>.jpg` for every archived clock (346 at 11:12 PT
  10-19; the coordinator shoots the later ones on box1).
- Census: `designs/clocks/census/` (small boxes committed); larger runs in ~/clock-census and
  ~/clock-census-p3 on devvm53196 (not committed: clocks.tsv is huge).

## State of the asks (2026-10-19)
- Done: shots of every archived clock + gallery-only catalog; composite rule; dedupe; gt + real
  time everywhere; orange-glass marker; verify horizon/positions/memory; name collisions.
- `clock shot --grid`: ~1.3 s per clock for camera + capture; opt-in until validated at full
  scale (`clock shot-compare /tmp/grid-all --against designs/clocks/img` gives SSIM per picture:
  good frames ~0.85). Known failure modes fixed: client timeout while worlds build (worlds are now
  built first), frozen level not sending chunks (batches <= 112x112, camera enters each batch from
  another level), blank / lost-connection frames re-taken per sandbox automatically.
- P <= 3 question (vjeux): `clock census --palette full --max-period 3 --warmup 60`: 0 clocks in
  2x1x1 k2, 3x1x1 k3, 2x2x1/2x1x2/3x2x1/3x1x2/2x2x2 k4, 3x3x1/3x1x3 k3 (the same pipeline with
  --max-period 4 finds the P4 clocks: the pruning is sound). Bigger boxes were running.

## Next
- Full-scale --grid validation, then make it the default.
- P <= 3 at 3x3x1 k4, 3x3x2 k3/k4, 3x2x2 k4 -> an `impossible.tsv` entry ("no P <= 3 clock with
  <= k blocks in <box>, full palette, self-start or one block placed last").
- Core census re-run with the mirror type key + `census-report` into designs/clocks/census/README.md.
- `clock video` per clock + montage per period on one client (the coordinator records).
