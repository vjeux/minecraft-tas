# Track boxproof: whole-box bounded model checking of flat doors

Question (vjeux): can the search ever be exhaustive over ALL the redstone a flat piston door
can use? Today's pipeline is exhaustive only inside its decomposition (mechanism catalog, then
static wiring). This track builds the one-problem check: every cell of a W x H x 1 box is a
variable (one-hot over a palette), the game is unrolled tick by tick for the open trace (lever on
from rest) and the close trace (lever off from the open state), both from one placement, and a SAT
solver answers "does any filling of the box form a door within T ticks per edge?".

Code: `crates/piston-door/src/boxproof/` (`mctas door boxproof fuzz|case|bmc|catalog|batch`).
* `bmc --w W --h H --dx X --dy Y --t T` whole box; `--fix design [--fix-kinds piston,stone]`
  fixed-pistons mode (those cells fixed, the rest free, full palette incl. extra update
  pistons); `--fix-all --free "x,y;.."` a partial placement; `--prove` (races forbidden),
  `--nondet` (races = either outcome), default find mode (races ignored, CEGAR); `--ring-rest`
  (ring plain at rest too: the brief's reading) else FULL seamless = ring plain when open (as
  `door properties`); `--lever-mod K --lever-rem k` lever cubes; `--cfg sat|unsat` CaDiCaL presets.
* `fuzz --count N --w 5 --h 5` the model vs redsim on random boxes; `case --k N` one in detail.

## The model (`state.rs`, `tick.rs`, `round.rs`)
* Written ONCE over a Boolean algebra (`alg.rs`): `Conc` = plain bools (the reference model the
  differential tests run), `Sat` = hash-consed Tseitin gates in CaDiCaL (rustsat). A model that
  matches redsim on bools IS the circuit the solver searches.
* Palette: air, stone (door block, conductor), obsidian, glass (non-conductor), redstone block,
  dust (exact power 0-15), repeater 1-4 E/W, observer E/W/U/D, torch, wall torch E/W, note block
  (powered at each neighbor update + 6-neighbor update; instrument from the block below), lamp
  (lit at once, unlit 4 gt later), piston / sticky piston E/W/U/D (+ heads, extended at rest),
  moving blocks (block-entity content, extending, source, progress), one lever: in the box, just
  outside it, or out of the plane (`--oop`), its support never moving.
* Signals per checkpoint (exact): strong/weak conductor power, dust as the unique fixpoint (no
  wire signals while a wire computes; step diagonals with the conductor rules), piston power
  (5 sides + quasi-connectivity), repeater / torch / note / lamp inputs, full-face sturdiness.
* A tick, like `ServerLevel.tick`: (1a) due repeater ticks, (1b) due observer, torch and lamp
  ticks, (2) block events in ROUNDS (events queued while a round runs execute in the next; 2
  rounds), (4) moving blocks age / place. After every step `settle`: dust recomputed, changed
  cells, neighbor updates with the game's exact patterns (setBlock 6 neighbors; dust change =
  distance 2; torch toggle = distance 2; repeater/observer = front + its neighbors except back;
  lever = own + support; piston moves per moved cell / arm / base; heads forward to the base;
  note blocks at once), observers fire on any change of the watched cell (unpowered, no tick
  pending), repeaters / torches schedule on a mismatch when updated (not when collected this
  tick), pistons check when updated (QC is a BUD) and queue events (extend only if the push
  resolves now; retract-and-drop when the block two ahead is still extending). Pistons: straight
  structures with the 12 limit, sticky pull, spitting, retract while the head still moves, pushed
  pistons and observers (a pushed observer facing its destination leaves a stale tick; placed
  observers fire or turn off).
* Inside a step the model is PARALLEL: where the game's order could change the outcome it raises
  a FLAG: a repeater/torch/lamp input changing within its own phase, an observer turning off
  while its watched cell changes, a component checked before dust settled (lever cascade BUD),
  two events writing one cell, an event whose power changes within its round, events left after
  the last round. `--prove` forbids flagged runs; `--nondet` makes the first four an either-way
  choice (a fresh variable: sound) and forbids the rest; find mode ignores them (CEGAR).
* Requirements (sound: a real door never violates them): nothing pushed out of the box, nothing
  destroyed (a drop is an entity: the judge fails), dust/repeaters/torches on full sturdy faces.
* Door: 9 door cells stone at rest and air when open; rest stable (repeaters/torches/notes/lamps
  consistent, dust exact, pistons extended iff powered or powered-but-blocked); open idle at T;
  close idle at T at exactly the rest state; FULL seamless as above.

## Validation (differential, `door boxproof fuzz`)
Random 5x5 boxes from the whole palette, settled in redsim, then 30 gt lever on + 30 gt lever
off, compared after every tick with the bool model: every block state, moving-block entity
(content, progress), comparator output, pending scheduled tick and pending block event.
* With bulbs and comparators (4c73b6b), seeds 11-15, 100,000 boxes: ~42,700 compared (~3,100
  with pistons moving), 9 UNFLAGGED mismatches (0.02%): piston events queued in the
  moving-block phase / close-phase piston chains (5), observers seeing a dust transient inside
  one phase (2), a bulb whose input fell and rose within one phase (1), other (1).
* Earlier palette (seed 11, 20,000 boxes): 7,691 compared, 6 unflagged.
* Bugs the fuzz found and fixed: the lever's own update reaches its support; pushed observers'
  stale ticks; the note-block event kind; copper bulbs are NOT redstone conductors. A
  calibration found the worst one: the torch phase dropped an observer's toggle in FREE cells
  (fixed 02:20Z, 40d9258; fuzz could not see it since its cells are concrete): every whole-box /
  fixed-pistons result before that commit is void.
* e89b with its placement given: SAT in 4 s (8x9, t=40/edge, 9.5M vars / 33M clauses), the model's
  door passes the redsim screen (open 34 / close 34). With ONE block-event round e89b is UNSAT:
  it needs events chained within a tick, hence 2 rounds. e89b with one wiring cell erased (palette
  observer+torch, after the fix): SAT 3.3 s.

## Results
Proofs (`--nondet`, strict ring at rest and open, T=20/edge, palette above, 2 event rounds):
| box | door offsets (mirror-reduced) | vars / clauses | result |
|---|---|---|---|
| 5x5 | all 6: (0,0) (0,1) (0,2) (1,0) (1,1) (1,2) | 1.4-2.6M / 4.8-9.3M | UNSAT in 451, 66, 284, 105, 0.0, 50 s |
| 6x6 | 4 of 8 started | 4.4M / 16M | see log (30 min cap) |
Even 5x5, where no door can fit, costs the solver minutes per offset: the temporal unrolling is
the cost, not the encoding size.

Find mode (`sat` config, CEGAR via the redsim screen, 1 position while searching), all after the
fix, on box2: e89b's mechanism in 7x8 (x -2..4, y -3..4; 4 lever cubes), vmid vb3 (2 cubes),
vb2, vc1, wire64 f131L, families 8x8-131 (fits 7x8), wire64b w63 top region in 7x9, whole box
7x8 door (2,2): no answer in 35-50 min each; stopped at 03:47Z by the frame-12/12 target
correction (none of those mechanisms is 12/12). Each instance ~13.5M vars / 49M clauses / 12 GB.

wire131 f131F-b2 (7x9, frame 12/12 with 3 static stones, one ideal input left: N's train at
(-2,4)), exhaustive with the real simulator (`door cosearch search --enum-k K --focus-cells`,
palette `all` = 56 states, 1-position 2-cycle screen, lever fixed):
| free cells | max changed | screens | time | result |
|---|---|---|---|---|
| the 3 pocket cells (-2,4) (-2,5) (-1,5) | 3 | 175,615 | 2 s | none, best 77/79 |
| pocket + (-1,4) (-2,3) (0,5) (0,4) (-2,6) (-1,6) | 3 | 14.1M | 33 s | none, best 77/79 |
| the same 9 | 4 | 1.167 B | 21 min | none, best 77/79 |
| the same 9, PERIODIC rest rule (main 96a62d6, screen 2cd7dc3) | 4 | 1.167 B | 16 min | none |
| the same 9, lever on the front face of (-2,6), (-1,5), (-1,4), (-2,3), (0,5), (0,4), (-2,5), (-2,4) | 3 | 14.1M each | ~11 s each | see log |
(the k=5 run over 7 cells with a 39-state palette was stopped for the periodic re-run.)
boxproof (bulbs + comparators exact) with all 9 cells free (any number changed): t=48 without
split rounds and t=40 with `--split-rounds`: no answer yet.

## Known-answer calibration: the community 7x9 (c79, tracks/c79)
c79-closed (lever ON at rest, droppers, a normal piston firing into an empty cell, dust steps),
every block fixed: UNSAT in the model at first; `door boxproof replay` (the design through the
model and redsim tick by tick) showed why: two block events of ONE round meet at (1,-1) (the
corner puller (-1,-1) pulls BM out, the bottom puller (1,-2) extends into the vacated cell, in
queue order). `--split-rounds` (each round in two sub-rounds, the solver picks which pending
events go first) admits it: c79 fully fixed SAT in 8.3 s, redsim screen Pass (open 14 / close 14).
Then pockets of c79's wiring freed (full wiring palette, `--lever-on --frame12 --split-rounds`,
t=22 per edge):
| freed cells | vars / clauses | result |
|---|---|---|
| A3: the 3 hidden top-left dusts (-2,4) (-1,5) (0,4) | 11.5M / 40M | SAT 628 s: exactly c79's dust, screen Pass |
| B5: top-right dropper (4,3), observers (2,4) (3,4) (3,3), dust (4,4) | 12.7M / 44M | SAT 1262 s: c79 with a note block instead of the dropper at (4,3), screen Pass |
| A3 + B5 | 13.7M / 48M | running |
| C6: bottom chain (4,-1) (3,-1) (2,-3) (1,-3) (0,-3) (-1,-3) | 13.0M / 45M | SAT 886 s (screen: door not restored), then 0.1 s later a model that passes: droppers at (1,-3) and (3,-1) instead of two smooth stones |
(B5 and C6 first came back UNSAT: the comparator tick wiped a free cell's observer pulse; fixed
in fc86166, the third bug of that pattern, found only because the answer was known.)
(at t=50 the same pockets were 30-35M vars and gave no answer in 15-30 min: keep T near the door's
own open/close time.)

## Sizes
~515k clauses per tick for a free 7x8 box, ~270k with fixed pistons (per-cell round, `round.rs`:
every cell takes at most a handful of candidate new states, combined as a sum of products).

## Coverage (what an UNSAT proves)
| aspect | status | over/under |
|---|---|---|
| palette | the list above | blocks outside it (bulb, comparator, trapdoors, dropper, slabs, leaves, rails, buttons, slime) not covered |
| supports | full sturdy faces (stone, obsidian, glass, rblock, observer, note, lamp, retracted piston, extended base back, head front) | UNDER: CENTER supports (torch on a sideways base's top), moving blocks as supports |
| dust shape | power only (in a plane every non-dot dust powers E, W, down) | dots excluded; observers watching a dust SHAPE change without a power change missed (UNDER) |
| order inside a step | parallel + flags | `--nondet`: 4 race kinds either-way (sound), 2 forbidden (UNDER); `--prove`: all forbidden |
| block-event rounds | 2 | doors needing 3+ chained rounds in one tick excluded |
| torch burnout | not modeled | doors relying on burnout excluded in effect |
| horizon | T per edge, idle at T | slower doors not covered |
| residual mismatches | 6 / 7,691 random runs | where the model and redsim differ unflagged, an UNSAT may be spurious |
So an UNSAT here reads "no door in this palette, box and horizon, under this model": exhaustive
over the placement space, not yet a theorem about the game.

## Log
* 23:30Z start; 00:05Z model + diff harness; 00:20Z e89b re-found from its placement.
* 00:40Z families 7x8-006..033 batch killed (6 x 28 GB on the shared box; 008-017 known dead).
* 00:57Z per-cell round (2.3x smaller); 01:05-01:15Z glass, note blocks, lamps; lever-support fix.
* 01:20Z docs/coverage.md boxproof column; 01:35Z `--nondet`; 01:42Z CaDiCaL `sat` config.
* 02:00Z box2 (devvm53196) attached; 02:20Z the observer/torch free-cell bug found by a
  calibration (one erased cell UNSAT) and fixed; every run restarted on box2.
* 02:35Z 5x5 all offsets UNSAT (`--nondet`, T=20).
* 02:51Z find-mode calibration: e89b with 2 bottom wiring cells erased, full palette (pistons
  allowed in the free cells): SAT in 617 s, the solver put back e89b's two observers (screen
  Pass). 1 erased cell (observer+torch palette): 3.3 s. 6 erased cells (wiring palette): first
  model after 1130 s, then CEGAR: 4 models the redsim screen rejected (the model said open,
  redsim did not: race outcomes the find mode ignores), each next model in 2-15 s, the 5th
  passed (open 34 / close 34). 3 erased cells (full palette), e89b's mechanism in 7x8 (4 lever
  cubes + a wiring-only run), vb3 (2 cubes), vb2, vc1, f131L, 8x8-131, w63 top (7x9): no answer by
  03:15Z (35-50 min each). f131X stopped (wire131 proved its top link unwirable in the plane).
* 03:10Z "e89b squeezed into 7x8" (x -2..4, y -3..4: e89b's mechanism and its in-box wiring
  fixed, its cut column x=-3 and row y=-4 gone, 25 cells next to the cuts plus every air cell
  free, wiring palette): one run with the lever just outside the box under (0,-3), three lever
  cubes with the lever free. `door boxproof watch` (background) waits for a screen Pass.
* 03:45Z wire131 f131N-live1 (7x9) pockets: `--approx` (comparator -> 2 gt repeater, trapdoor /
  bulb -> glass, crying obsidian -> obsidian, dot dust -> dust) with the screen on the ORIGINAL
  design (only freed cells from the model; the merge path re-checked on e89b: Pass).
* 03:47Z TARGET CORRECTION (vjeux): when OPEN all 12 frame cells must hold plain blocks (FRAME
  12/12); every mechanism we had is 9-10/12, so every find run was stopped (both boxes). New
  `--frame12` (ring cells plain and non-air when open); check: e89b's placement with
  `--frame12` is UNSAT in 5 s (it is not 12/12), as expected. The whole-box mode needs no
  mechanism: `bmc --w 7 --h 8 --frame12` asks directly whether any 12/12 door fits.

## Next ideas
* The solver, not the encoding, is the wall: cost grows steeply with free cells (1 cell
  seconds, 2 cells with pistons 10 min). Stronger SOUND pruning first: a door cell changes only
  if a piston line can reach it (reachability of pistons over time, over-approximated);
  per-cell kind restrictions from the mechanism (in fixed-pistons mode a free cell crossed by a
  fixed piston's line can only hold air or a movable block); bound the number of pistons /
  observers; symmetry breaking where the model is mirror-symmetric (it is, without position
  order); seed the lever's region.
* Abstraction refinement: solve a coarse model first (ideal-input mechanism level with wiring
  reachability as necessary conditions); UNSAT there is a proof, SAT gives the timing skeleton to
  constrain the exact model.
* Incremental horizon (assumption literals per tick) and learned-clause reuse across door
  offsets; a parallel portfolio (CaDiCaL configs, kissat) per instance on the 166-core box.
* Exactness: the 6 residual fuzz mismatches (phase-4 placement order, a lamp re-schedule, a
  close-phase piston chain), event write conflicts as either-order choices, dust shapes, CENTER
  supports, more palette (bulb, comparator, trapdoors, dropper, slabs, leaves).
