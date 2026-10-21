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
## Fast (9 gt/block horizontal)
- fly-horizontal-7b-9gt-e75c5f (tracks/fly/fast, archived, robust): e2cf52 without its rear
  observer; a redstone block rides on the front part (on a second slime) beside the cell the rear
  piston lands in. The piston fires as it lands (onPlace sees the power: 1 gt) instead of waiting
  for the rear observer (2 gt after landing): push t1, front lands t3, front observer t5, the
  sticky puller reach-pulls t5..t7, rear lands t9, pusher fires t10: 9 gt. The redstone block must
  not quasi-connect the pusher once the front has moved (above the pusher it does: the pusher
  stays extended and the pull breaks the machine), so it sits beside it. In e2cf52 the redstone
  block needs a slime of its own (the front slime is the block the pusher pushes, a neighbour of
  the landing cell); a 6-block version needs a host whose front slime sits diagonally ahead of the
  landing cell (the k = 6 census with redstone blocks decides).
- The up-flyer's 9 gt is the same 1 gt, by quasi-connectivity: the front observer watches the cell
  the rear piston lands in and powers the slime two above it.
- `grow --add 2 --faster` (+ redstone block, glazed terracotta) on both 9 gt machines: nothing
  faster. `grow --remove 1 --add 1|2 --faster` (one block taken out, then grown) on every 6-block
  machine: UP 6 blocks 9 gt/block (fly-up-6b-9gt-0e4df1, -339615, robust: a redstone block on the
  honey part, no rear observer) and 82 horizontal 7-block 9 gt/block candidates (30 types, 13
  captions): the robust ones are the redstone-block-on-a-slime idea again (near-duplicates of
  e75c5f, not archived); a second idea (fly-horizontal-7b-9gt-aa517c, archived, east-west only): a
  redstone block on a slime stands diagonally above the pusher (quasi-connected) and a third,
  sticky piston under that slime lifts it away and lowers it back. Nothing below 9 gt/block. What 9 gt is made of: push 2 + observer 2 + reach-pull 2 + pull 2 + landing fire 1. The
  pusher retracts only after the front lands (it misses the redstone block leaving: its state turns
  extended after its own push's neighbour updates), so the pull cannot start before t7.

## Census with redstone blocks (k = 6, 14 boxes, 67 min)
- 678 machines, 233 census types: UP 6 blocks 9 gt/block (5 types archived with the 2 from the
  swap search, all robust), DOWN 6 blocks 10 gt/block in EVERY orientation (8 types, robust: the
  redstone block replaces the rear observer; the observer-only 6-block down flyers are east-west
  only; fly-down-7b-10gt-aca731 is now dominated and out of the archive), horizontal 6 blocks 17
  gt/block (dominated); no horizontal 6-block 9 gt/block, nothing below 9 gt/block.

## Vertical two-way
- Every orientation of the 7 reversible horizontal engines (`orient --all`, then `two-way
  --reversible-only` trying every kick cell): vertically only DOWN flies, both ways never; the up
  flyers with two sticky pistons (6 census types) break when kicked from the other end.
- `grow --add 2 --both-ways` on those 6 up flyers: 7-block engines that fly up AND down (3 types,
  tracks/fly/vertical-rev, robust both ways in the real game): the 6-block up flyer 4c4084 (or
  a62c63) plus one slime on the top part beside its observer. Return stations: at the top 2 blocks
  are enough (an obsidian ceiling + one observer); at the bottom none with k <= 3 (radius 1:
  obsidian/observer/dust, then + stone, repeaters, redstone block; radius 2, k <= 2, + torches),
  on every reversible orientation. Why the bottom is hard: the reversing sticky piston faces up
  (its back is the floor cell), every observer that can watch a cell the machine enters only at
  the stop points its output away from that piston, and an observer next to a slime's final cell
  is dragged away by the reversal (tried by hand: vtw-test, a lamp in the cell the rear observer
  watches, broken for that reason). The 73cdd4 trick (a station observer and the rear observer
  watching each other, vtw-test2: floor + 1 observer) does reverse the machine, but the observer
  pair keeps pulsing every 4 gt and the next pulse extends the reversing piston while the other
  part pulls: the machine splits. A bottom station needs a one-shot pulse there: 4+ blocks (a dust
  line, or a way to stop the pair after one pulse): the next thing to build for a vertical
  two-way.

- No other speed than 10 gt/block horizontally at 6 blocks (k <= 7 in 4x2x1, k <= 6 in 14
  boxes, grow +1).

## Speed ladder (grow) — dominated, NOT archived
- `grow --add 2` on every archived 6-block machine (8 blocks): 11, 12, 16, 17 gt/block
  horizontal and 16 gt/block up (162 candidates, 145 types); `grow --add 1` on those (9 blocks):
  18, 19, 24, 25 gt/block. One per caption (96) all passed in the real game, but every one is
  bigger AND slower than the 6-block 10 gt/block machines of its direction: dominated, so taken
  back out of the archive (coordinator, vjeux: "we don't need 20 variations of the same design");
  kept as a record in tracks/fly/speed (8 blocks) and tracks/fly/speed9 (9 blocks). The two extra
  blocks are observers (and a slime) that watch the other part or the first observer: the puller
  stays powered longer (a pulse stretched to 4 gt = +2 gt), or both sticky pistons fight and the
  machine ratchets (11 gt: it steps back and forth, net one cell per 11 gt).
- The Pareto front (blocks vs speed) per direction: horizontal 6 blocks 10 gt/block; up 6 blocks
  10 gt/block and 7 blocks 9 gt/block; down 6 blocks 10 gt/block (east-west only) and 7 blocks 10
  gt/block robust. Faster than 9 gt/block: not found yet.

## Payload
- fly-payload-e2cf52 family (base tracks/fly/payload/fly-payload-n1.txt, generator `mctas circuit
  fly gen payload ... --n {n}`): the 6-block engine fly-horizontal-6b-10gt-e2cf52 carrying n = 1..18
  blocks, 6 + n blocks, 10 gt/block for every n (judged n = 1, 2, 3, 9, 18 robust): a slime column
  up from the front slime (the top block gold), 9 at most (a piston moves at most 12 blocks: the
  3-block part + 9; n = 10 on one part never moves), then a column down from the rear slime; a 10th
  block on the rear: the pull fails and the machine breaks. fly-payload-9-15b and -18-24b are its
  members, archived first.
- Train family fly-train-e2cf52 (base tracks/fly/train/fly-train-n1.txt, `mctas circuit fly gen
  train --n {n}`): the same engine pulling n trailers, 5 + 3n blocks, 10 gt/block for every n
  (judged n = 1, 2, 3, 8, 16 robust; 16 trailers = 53 blocks). A sticky piston on the rear slime
  (facing back, fired with the pusher by the rear observer) reach-pulls trailer 1 each cycle; each
  trailer but the last carries a sticky piston facing back and a rider observer that pulls the next
  trailer (it starts against that piston's face, so the gap opens when its puller moves). Each
  trailer is a move of its own (12 blocks at most), so the size, and the payload, have no limit
  (a two-part machine moves at most 24 blocks).

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
- `two-way` on all 7 reversible engines (01ce95, 02a7fb, 67f769, 73cdd4, 873917, 91737e, 964ac6):
  no station of a wall + 2 blocks within TWO cells of the stop (palette obsidian, stone,
  observers, repeaters, cross dust, redstone block, torches), so 4 station blocks is the minimum;
  k = 3 (radius 1, obsidian/observer/dust): 01ce95 and 73cdd4 get both stations
  (fly-two-way-14b-73cdd4: a station observer facing down meets the front observer facing up at
  the stop, they watch each other and their pulses work the engine back; round trip 200 gt over
  9 positions, robust); 91737e and 964ac6 only a station ahead; 02a7fb, 67f769, 873917 none.

## Real use: start levers, docking, ground (2026-10-21)

Coordinator: give every kind a real switch (one-way: "start: flip the lever"; docking: off docks
it), judge it under several flips, then minimum volume per kind; clk-misc (judge audit) agreed the
bar: off at rest = parked and idle, a flip launches it once, extra flips at any phase never break
it or strand a block, the lever where a player reaches it (on the station, off the corridor).

- **Family A** (`circuit fly start-scan`): the lever in the cell the kick observer watches, hung on
  a stone off the corridor and away from slime. 0 of every archived machine passes the 4 gt flip:
  the kick observer sits on the part that stays (it fires its own part's piston through the slime
  it is stuck to), so it still watches the lever when the lever goes off 4 gt later and fires the
  pusher in the middle of the pull (fly-d1-lever, the selftest's cal-fly-start-watcher-stays).
  Up/down ones also fire at placement (their kick cell is the lever's own update).
- **Family B** (`--watcher`): an observer W on a piston's side or back, its back on that piston,
  stuck to the machine by up to 2 slimes, the lever in the cell W watches. When W rides the part
  that piston pushes, W leaves the lever 2 gt after the flip. First pass: fly-d1-w (11 blocks,
  4x2x4, robust). Over c6/c6all/layouts/vertical/... : volume 16 for many horizontal and down
  layouts (9 blocks), fly-up-7b-9gt-ff426b-w44 the only up one (16), payload-n1 36, train-n1 40.
  fly-h6-w (W on top of 01ce95's sticky pusher by hand) fails: A got power for an instant, its
  extension completed and the sticky retraction pulled the moved part back (the original spits it
  with a 2 gt pulse).
- **compact** on the start judge: every 16 seed -> 12, where the machine's own trigger observer
  watches the lever and rides the pushed part (no W: fly-horizontal-start-7b-10gt is 6 machine
  blocks + the stone); up 12 (9 blocks, 9 gt/block), payload 36 -> 18, docking 90 -> 64. Nothing
  smaller in 40-60k more moves from each 12. Most down machines are rotation-dependent (as the
  observer-only down types always were); 8136e6-w5 and ae5894-w9 are robust, 8136e6-w5 compacts
  to 2x2x3. train-n1's compact result is a plain 2-part machine with a passenger (the judge does not
  know trailers), so train is not a real-use record.
- **Docking loophole**: compact's first 4x2x8 "docking" machine made one trip per switch-on and
  parked with the lever on; flying_switch now wants a repeating round trip and motion in the last
  round trip before the off edge (cal-fly-docking-parks). Under it compact gives
  fly-docking-15b-78gt (stations 4 cells apart, a 3-block far station).
- **Ground clearance** (robust::fly_clearance, grass on dirt): slime or honey in the bottom layer
  drags the grass (fe361f, e75c5f, payload-n1, train-n1, fly-d1: 1 block of air); a down-facing
  observer in the bottom layer sees the grass under it turn to dirt on a random tick (redsim has no
  random ticks: fly-docking-15b docked on grass failed 8 of 16 game builds, cal-two-way-machine 1
  of 16) so those need 1 block of air too (01ce95, both docking machines). Flies along the ground:
  fly-horizontal-start-7b-10gt, fly-payload-18-24b. `circuit sim --ground N` runs one over it.
- **Start-lever payload family** (coordinator: the device-less payload family carries up to 18):
  family B on e2cf52's members fails from n = 9 up (the front part already pushes 12 blocks, W
  and its glue do not fit); fly-horizontal-start-7b-10gt adds no moving block, so each part still
  carries 9: fly-payload-start-7b, a column up from the rear slime, past 9 split evenly with an
  L-shaped second column on the front part (`gen payload --bend2 up --balance`); every member
  n = 1..18 robust in the game (tracks/fly/start/payload). A column on the front part cannot go
  down (the lever station is under it) or start above its sticky piston (it would touch the rear
  column), hence the L.
