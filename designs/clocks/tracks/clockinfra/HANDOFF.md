# clockinfra HANDOFF (the clock study's scaffolding and judge)

Owner of `crates/clock*` (except the explorers' own crates `crates/clock-{obs,rt,sculk,item}`),
`docs/clocks.md` (the one home: definition, judge, format, commands, census results) and
`crates/redsim`. Branch `explore/clockinfra`; everything is pushed there and merged by the
coordinator (f1a42990).

## What exists
- `crates/clock` (optimized library):
  - `Clock`: the door design format + `port output` + `port start: x y z (last | use [T] |
    <state> <T>) [at N]` + `placement: player` + `nbt:` lines + `extend` headers.
  - `sim`: exact redsim detector (`World::state_key` first repeat; the clock's period = smallest
    divisor over which every cell's full state repeats: block state + comparator strength +
    container contents); output = port, else the changing cell with the longest own period;
    COMPOSITE = output repeats faster than the whole AND independent spatial groups; families =
    in-place changes + moving redstone blocks; periods printed as `20 gt (1 s)`.
  - `verify`: the real-game judge, 4 positions x 4 rotations by default, horizon = W + 4P from
    redsim (<= 20000 gt), sparse frames + segmented bridge reads (big boxes, long horizons);
    design entities (`entity:` lines, minecart clocks) summoned after the blocks, turned with the
    design, checked at the end (no foreign entity, none lost or outside the box).
  - `wave`, `canon` (type key over 4 rotations AND mirror images + nbt; `chiral`), `catalog`
    (archive: SAME TYPE refusal per (period, type), `--dedupe`, aliases.tsv, name collisions ->
    `<name>-<6 hex type hash>`, family judging; CATALOG.md = gallery only + Extendable cards;
    coverage.md = matrix), `census` (exhaustive, resumable, MINIMAL flags, `--max-period` early
    pruning, `full` palette, `census-report`), `extend` (formulas, families, extend-check,
    extend.tsv), `shot` (real-client pictures, orange-glass output marker, two reused sandboxes:
    ~2 s a clock, dark/uniform-frame retakes, elongated close-ups; `--grid` experimental;
    `shot-compare`), `video` + `montage` (one client session for every clip).
- `crates/clock-cli` (unoptimized): `mctas clock sim | verify | archive | catalog | census |
  census-report | extend-check | shot | shot-compare | video | montage | obs | rt | sculk | item`.
- Calibration: `designs/clocks/calibration/` (8 clocks, robust in the game, redsim agrees);
  `door selftest` checks `clock:*`.
- Pictures: `designs/clocks/img/<archive stem>.jpg` for every archived clock (1582/1582 on
  2026-10-19 13:10 PT; `clock shot designs/clocks/archive/*.txt` takes only the missing ones, ~2 s
  each; `clock catalog` prunes pictures of designs no longer archived).
- Census: `designs/clocks/census/<box>-k<k>/` for 11 boxes (core palette, rotation+mirror type
  key) and `designs/clocks/census/README.md` (`clock census-report`).

## Results worth knowing
- P <= 3: no clock with the `full` palette (self-start or one block placed last) in any box up to
  3x2x2 / 3x1x3 with 4 blocks and 3x3x2 with 3 (impossible.tsv row; the game-source proofs for the
  redstone-tick and block-update families are clk-rt's and clk-obs's rows).
- Shot timing: the old 13 s a clock was `drop_sandbox` of a visited level (6-12 s) + a 5 s hold;
  two sandboxes reused in turn fix it.
- Type identity (`clock::ident`): sticky pistons that never pull = pistons, inert solids = stone,
  checked per design on redsim; the first dedupe with it removed 148 twins (all sticky/plain
  piston pairs). Keys cached in archive/type-keys.tsv (gitignored).
- Gallery: the dominance fold (a clock of the period with fewer blocks and a subset of the block
  kinds hosts it as a variant) shows ~1565 types + ~806 variants at 2373 clocks.
- Extendable cards: `### P = a + bn gt` heading, n_low and n_low+1 pictures (the n_low+1 one is
  `img/<base>.n<k>.jpg`, built by `clock shot` with the family's generator).
- Box warning: a 160-thread census on devvm53196 starved the agentcloud node agent twice (the node
  stopped announcing); keep jobs <= ~100 threads / half the RAM. While devvm53196 was down I
  worked in my own clone on box1 (devvm55865, <= 32 threads).

## Next
- Explorers convert their families to `extend` headers (clk-rt has generators: `mctas clock rt
  family <name> --n N --out F`); archive them with `clock archive` (judged n recorded in
  extend.tsv; the catalog drops recorded instances from the per-period gallery).
- Archive tables: one row per file (read_rows collapses repeated rows from union merges; dedupe
  never removes a file as its own twin): if a merge repeats rows again, `clock archive --dedupe`
  repairs the tables (reports COLLAPSED n).
- Minecart (entity) clocks: the cart does not show in `clock shot` pictures (frozen level: the
  entity is not sent to the client); the rails do.
- P <= 3 at 2x2x2 k5 / 3x3x2 k4 if anyone needs a stronger bound (hours; <= 100 threads).
