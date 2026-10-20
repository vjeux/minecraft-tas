# Track fly: flying machines (contraption study 7)

Brief: /home/vjeux/agents/CONTRAPTIONS.md (study 7). Self-propelled slime/honey + observer +
piston structures. Kinds: one-way engines (fly until blocked), two-way (return stations /
bouncing), with payload; horizontal and vertical. Axis: kind + speed (gt per block moved);
metric: blocks (fewest first). Every design verified in the REAL game (4 positions x 4 rotations).
Archive: designs/circuits/fly/archive/ (`mctas circuit archive`, the shared judge carries this
track's tracker as `crates/circuit/src/fly.rs`).

## Tools (crates/circuit-fly, `mctas circuit fly ...`)
- `sim <designs>... [--trace N] [--visits]`: redsim, milliseconds. Places like the clock judge,
  kicks (`port start:`), tracks the translation every gt (structural states, piston heads part of
  their piston, a frame with a moving block = in motion) and the steady state (frame n+P equals
  frame n shifted by d, held for whole periods). FLIES / SHUTTLES / MOVES / STILL / BROKEN, speed
  P/d gt per block, first move, end state (at rest intact at a shift, corridor end, horizon).
- `verify <designs>... [-v]`: the same tracker on the real game, every build in its own corridor
  sandbox, 4 positions x 4 rotations, entities at the end = a drop = BROKEN; `robust` when every
  build agrees (direction turned back); then redsim's verdict on the same design.
- `census --box AxBxC[,...] --k K [--kmin] [--palette] [--procs 8 --workers 12] --out DIR`:
  exhaustive redsim search: every face-connected filling of each box with kmin..=k blocks of the
  palette (default slime, honey, stone, observers, pistons, sticky pistons, every facing), one per
  rotation class, every observer feeding a piston or a conductor, kicked once per observer (1 gt
  stone in its watched cell, or that cell's block placed last), every quarter turn tested (Java's
  update order makes some machines fly in one orientation only). FOUND = steady motion for 2+
  periods AND the design's own shape recurs at 2+ places (intact). DIR/found.tsv + one design per
  type in DIR/designs/. Use child processes (`--procs`): one process with ~100 threads spends 3/4 of
  its time in glibc's malloc arena locks.
- `types <designs>...`: type identity (normal form by redsim-verified swaps: sticky->piston,
  honey->slime, slime/honey->stone, observer->stone; then rotation + mirror) and the dominance fold.
- `prep <designs>... --out-dir DIR`: readable names (fly-<class>-<blocks>b-<speed>gt-<type hash>)
  and a caption (pistons and observers by facing relative to the travel) for the archive.

## Results
- MINIMUM = 6 BLOCKS (census, redsim, every quarter turn): no flying machine with <= 5 blocks in
  3x2x1, 4x2x1, 2x2x2 (k <= 6), nor 6 blocks in 3x2x1. 6-block machines: 26 horizontal types, all
  10 gt/block, all robust in the real game (designs/circuits/fly/archive/, 2026-10-19): two parts
  of 3 blocks (observer + slime + piston each); the rear part's piston pushes the front part one
  cell, the front observer (riding) fires 2 gt after landing, the front's sticky piston reach-pulls
  the rear slime (dragging the rest of the rear); the rear observer fires the pusher again. Variants
  differ in where the observers watch (back / forward / up / down / sideways), honey in place of a
  slime, or a sticky pusher (spits with the 2 gt pulse).
- 4 vertical (down) 6-block types fly 10 gt/block only when the structure lies east-west
  (rotations 0 and 2); turned north-south they oscillate in place (period 7 gt): Java's update
  order. The shared judge refuses them (a failing build); kept in tracks/fly/c6/.
- fly-d1 (7 blocks, my first derivation, rear X + 2 slimes + observer, front observer + slime +
  sticky Y): archived; a variant of the 6-block hosts.
- No other speed than 10 gt/block yet (k <= 7 in 4x2x1; bigger boxes running).
