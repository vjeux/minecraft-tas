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
- `prep <designs>... --out-dir DIR [--skip-archived DIR]`: readable names
  (fly-<class>-<blocks>b-<speed>gt-<type hash>) and a caption (pistons and observers by facing
  relative to the travel) for the archive; `--skip-archived` leaves out types already archived.
- `orient <designs>... --out-dir DIR`: the 24 cube orientations on redsim (class, speed, forward or
  BACKWARD); writes the vertical ones.
- `grow <designs>... --add 1|2 --out-dir DIR`: one or two blocks added anywhere around each design
  (every palette state), every quarter turn; keeps the ones that fly at another speed.
- `gen payload --base <design> --from x,y,z --dir up --n N [--cap 9 --from2 x,y,z --dir2 down]`:
  the payload family's generator.
- `station <machine> --k 3 [--palette ...] --procs 8 --out-dir DIR`: return-station search (below).
- `two-way <machines>... --out-dir DIR [--reversible-only]`: for each reversible machine (kicked
  from its back-facing sticky piston by a 2 gt redstone block it flies backward) the station search
  ahead and behind, then station pairs composed into one design, kept when it shuttles.

## Results
- MINIMUM = 6 BLOCKS (census, redsim, every quarter turn): no flying machine with <= 5 blocks in
  any box up to 6 cells long (14 boxes: 3x2x1 ... 2x2x2, 3x3x1, 2x3x2, 3x2x2, 2x4x1, 4x1x2, 3x1x3,
  5x2x1, k <= 6: 277 machines, 140 census types, 68 after the redsim-verified swaps of `types`).
  Archived: 47 horizontal 6-block types, all 10 gt/block, all robust in the real game; 3 up
  6-block types robust; 17 down 6-block types, all east-west only (rotation-dependent). Two parts
  of 3 blocks (observer + slime + piston each); the rear part's piston pushes the front part one
  cell, the front observer (riding) fires 2 gt after landing, the front's sticky piston reach-pulls
  the rear slime (dragging the rest of the rear); the rear observer fires the pusher again. Variants
  differ in where the observers watch (back / forward / up / down / sideways), honey in place of a
  slime, or a sticky pusher (spits with the 2 gt pulse).
- `grow --add 1` (every 6-block archive machine + one block anywhere around it, every state, 4
  quarter turns): 21756 candidates, none flies at another speed than its base.
- 4 vertical (down) 6-block types fly 10 gt/block only when the structure lies east-west
  (rotations 0 and 2); turned north-south they oscillate in place (period 7 gt): Java's update
  order. Archived as rotation-dependent (the judge passes them flagged since af4104864).
- fly-d1 (7 blocks, my first derivation, rear X + 2 slimes + observer, front observer + slime +
  sticky Y): archived; a variant of the 6-block hosts.
- VERTICAL by tilting (`orient`: the 24 orientations of every archived machine on redsim, then
  the real game): UP 6 blocks 10 gt/block robust (2 types: fly-up-6b-10gt-2901e6, -ca150a); UP 7
  blocks **9 gt/block** robust (fly-up-7b-9gt-ff426b = fly-d1 tilted up: the rear lands at t10 in
  the block-entity phase while its pusher is already powered, so it fires at t11 by onPlace
  (landing-fire) instead of waiting for its observer: an odd cycle); DOWN 7 blocks 10 gt/block
  robust (fly-down-7b-10gt-aca731 = fly-d1 tilted down); 7 more 6-block down types, all
  rotation-dependent (east-west only). Tilting a horizontal machine to fly down gives
  east-west-only machines in every 6-block case found.
- No other speed than 10 gt/block horizontally at 6-7 blocks (k <= 7 in 4x2x1, k <= 6 in 14
  boxes, grow +1).

## Speed ladder (grow)
- `grow --add 2` on every archived 6-block machine (8 blocks): 11, 12, 16, 17 gt/block
  horizontal and 16 gt/block up (162 candidates, 145 types, one per caption archived: 84, all
  pass in the real game: 11 gt 13 (2 rotation-dependent), 12 gt 16, 16 gt 32, 17 gt 21 (12
  rotation-dependent), up 16 gt 2). The two extra blocks are observers (and a slime) that watch
  the other part or the first observer: the puller stays powered longer (a pulse stretched to 4 gt
  = +2 gt), or both sticky pistons fight and the machine ratchets (11 gt: it steps back and forth,
  net one cell per 11 gt).
- `grow --add 1` on the 8-block ones (9 blocks): 18, 19, 24, 25 gt/block (12 archived, 24 gt
  robust, the others rotation-dependent).
- So far: 9 (up, 7 blocks), 10 (6), 11, 12, 16, 17 (8), 18, 19, 24, 25 (9) gt/block.

## Payload
- fly-payload-e2cf52 family (base tracks/fly/payload/fly-payload-n1.txt, generator `mctas circuit
  fly gen payload ... --n {n}`): the 6-block engine fly-horizontal-6b-10gt-e2cf52 carrying n = 1..18
  blocks, 6 + n blocks, 10 gt/block for every n (judged n = 1, 2, 3, 9, 18 robust): a slime column
  up from the front slime (the top block gold), 9 at most (a piston moves at most 12 blocks: the
  3-block part + 9; n = 10 on one part never moves), then a column down from the rear slime; a 10th
  block on the rear: the pull fails and the machine breaks. fly-payload-9-15b and -18-24b are its
  members, archived first.

## Two-way (return stations)
- The reversible engine: the 6-block types with TWO sticky pistons (e.g. fly-horizontal-6b-10gt-
  01ce95): each part's observer powers only its own part's piston at rest (the other piston sees
  the slime through its front face), so whichever piston fires first sets the direction: kicking the
  back-facing one first flies it backward (tested: rev-01ce95, 10 gt/block north).
- `station` search (`mctas circuit fly station <machine> --k 3 --procs 8`): every obsidian wall cell
  in front of the leading face + up to k station blocks around the stopping place; nothing with
  k <= 2 (radius 1), 4 stations with k = 3, all one idea: an observer watching the cell ONLY the
  trailing observer enters at the stop (its final cell: no earlier passage) fires 2 gt later into a
  block under a cross dust that points into the BACK of the leading part's sticky piston; that
  piston pushes the trailing part back with the 2 gt pulse (spit) and the engine runs the other
  way. The other end: the same idea mirrored by hand (observer under the leading piston's final
  cell watching the trailing observer's final cell).
- fly-two-way-14b (tracks/fly/two-way, calibration cal-two-way-machine): engine 6 + 2 x 4 station
  blocks, shuttles between shift -1 and +4, round trip 98 gt over 6 positions, real game robust.
  Station blocks are design blocks marked `port station.<n>` (they count; the tracker leaves them
  out of the machine's shape): implemented in crates/circuit (lib.rs, fly.rs, check.rs: an in-place
  run that outlasts the moving runs wins = two-way) at clockinfra's request.
