# Track boxproof: whole-box bounded model checking of flat doors

Question (vjeux): can the search ever be exhaustive over ALL the redstone a flat piston door
can use? Today's pipeline is exhaustive only inside its decomposition (mechanism catalog, then
static wiring). This track builds the one-problem check: every cell of a W x H x 1 box is a
variable (one-hot over a palette), the game is unrolled tick by tick for the open trace (lever on
from rest) and the close trace (lever off from the open state), both from one placement, and a SAT
solver answers "does any filling of the box form a door within T ticks per edge?".

Code: `crates/piston-door/src/boxproof/` (`mctas door boxproof fuzz|case|bmc|catalog|batch`).

## The model (`state.rs`, `tick.rs`, `round.rs`)
* Written ONCE over a Boolean algebra (`alg.rs`): `Conc` = plain bools (the reference model the
  differential tests run), `Sat` = Tseitin gates in CaDiCaL (rustsat), hash-consed. A model that
  matches redsim on bools IS the circuit the solver searches.
* State per cell per checkpoint: kind (air, stone, obsidian, redstone block, dust, repeater,
  observer, torch, wall torch, piston, head, moving block, lever), facing (E W U D), sticky,
  extended, powered/lit, repeater delay, dust power (unary, exact 0..15), moving-block entity
  (content kind/facing/sticky/powered, extending, source, progress 0/0.5/1), pending scheduled
  tick (due in 0..7 ticks), queued block event (extend / retract / retract-and-drop).
* Signals per checkpoint (combinational, exact): conductors' strong power (repeaters, observers,
  torches below, lever on its support) and weak power (+ dust beside / above), dust levels as the
  unique fixpoint (sources with every wire signal off, horizontal + step diagonals with the
  conductor rules of `incomingWireSignal`), piston power (5 sides + quasi-connectivity incl. an
  out-of-plane lever), repeater input, torch input, full-face sturdiness for supports.
* A tick, phase by phase like `ServerLevel.tick`: (1a) due repeater ticks, (1b) due observer and
  torch ticks, (2) block events in ROUNDS (events queued while a round runs execute in the next;
  2 rounds modeled), (4) moving blocks age / place. After every step `settle`: dust recomputed,
  cells that changed, neighbor updates (exact patterns: setBlock 6 neighbors; dust change =
  cells within distance 2; torch toggle = distance 2; repeater/observer = front cell + its
  neighbors except back; lever = own + support's neighbors; piston moves = neighbors of every
  moved block's old cell, the arm, the base; head forwards to base), observers fire on any change
  of the watched cell (unpowered, no tick pending), repeaters/torches schedule on a mismatch when
  updated (not if collected this tick), pistons check power when updated (QC is a BUD) and queue
  events (extend only if the push resolves NOW; retract-and-drop when the block two ahead is still
  extending). Pistons: straight-line structure resolution with the 12-block limit, sticky pull of
  one block, spitting (final-tick of the block two ahead), retract while the head is still moving
  (source final tick -> air), pushed pistons and observers, an observer pushed toward its
  destination leaves a stale tick, placed observers fire (unpowered) or turn off (powered).
* Inside one step the model is PARALLEL: where the game's order could change the outcome it
  raises a FLAG (`flag()`): two events writing one cell, an event whose power changes within its
  round, a repeater/torch input changing within its own phase, an observer turning off while its
  watched cell changes, a component checked before dust settled (lever cascade BUD), events left
  after the last round. Prove mode forbids flagged situations; find mode ignores them (CEGAR).
* Requirements (a real door can never violate them, so they are sound pruning): nothing pushed
  out of the box, nothing destroyed (a drop = an entity = the judge fails), dust / repeaters /
  torches always on a full sturdy face, the lever's support never moves.
* Door constraints (`bmc.rs`): the 9 door cells stone at rest and air when open; rest state stable
  (repeaters/torches consistent, dust exact, pistons extended iff powered, or powered-but-blocked);
  open state idle at T (no tick, event or moving block); close trace ends idle at exactly the rest
  state; FULL seamless (the 12 ring cells hold only stone, obsidian or air at rest and open).
  Lever candidates: every in-box cell off the door/ring with each in-plane support, every cell
  just outside the box, optionally out of the plane (`--oop`).
* CEGAR: every SAT model is written as a design and screened by `redsim_screen` (the judge's
  checks on redsim at 32 positions); a failing placement is blocked and the solver asked again.

## Validation (differential, `door boxproof fuzz`)
Random 5x5 boxes (palette above, lever in box / outside / out of plane), settled in redsim, then
30 gt lever on + 30 gt lever off, compared after every tick with the bool model: every block,
moving-block entity (content, progress), pending tick and pending block event.
* 30,000 random boxes, seed 7: 10,915 compared (1,062 with pistons moving): 10,913 identical or
  flagged; 2 UNFLAGGED mismatches (same class: a piston updated by the lever before the dust it
  depends on settles, then not updated again; partially flagged since 23:15).
* seed 3: 7,452 compared, 4 unflagged (2 lever-cascade BUD, 2 phase-4 queue differences, under
  study); 49 flagged mismatches.
* e89b (8x9 record) with its placement given: model SAT in 14.5 s (t=45/edge, 37M vars, 123M
  clauses, build 84 s); the model's door passes the redsim screen (open 34 / close 34). With ONE
  block-event round e89b is UNSAT: it needs events chained within a tick, so 2 rounds are modeled.

## Sizes and solve times (find mode, flags off)
| instance | T/edge | vars | clauses | memory | result |
|---|---|---|---|---|---|
| e89b fixed placement (8x9) | 45 | 37M | 123M | 13-33 GB | SAT 14.5 s, screen Pass |
| e89b, 1 event round | 40 | 13M | 41M | 13 GB | UNSAT 144 s (1 round is not enough) |
| 7x8 fixed pistons (fm-7x8-002/006/012) | 40 | 27M | 85M | 21 GB | running |
| 7x8 whole box, door (2,2) / (2,3) | 36 | 10.3M | 36.7M | 11 GB | running |

Per-tick cost after the per-cell round (`round.rs`): ~515k clauses per tick for a free 7x8 box,
~270k for a 7x8 box with fixed pistons.

## Coverage (what an UNSAT would and would not prove)
| aspect | status | over/under |
|---|---|---|
| palette | air, stone (door block), obsidian, redstone block, dust, repeater 1-4 E/W, observer x4, torch, wall torch E/W, piston/sticky x4 (+ heads, moving) | blocks outside it are not covered: note block, glass, lamp, bulb, comparator, trapdoor, target (= stone in a plane), dropper... |
| supports | full sturdy faces only (stone, obsidian, rblock, observer, retracted piston, extended base back, head front) | UNDER: a torch on a sideways base's top (CENTER support) is excluded |
| dust shape | power only (in a plane every dust powers E/W/down; dots excluded) | dot dust excluded; an observer watching a dust SHAPE change (no power change) is missed (UNDER) |
| order inside a step | parallel + flags | prove mode forbids flags (UNDER: race-dependent doors excluded); find mode ignores them (neither; CEGAR) |
| block-event rounds | 2 | more chained rounds flagged |
| torch burnout | not modeled | doors toggling a torch 8x in 60 gt excluded in effect |
| horizon | T ticks per edge, idle at T | doors slower than T not covered |
| one position | the model has no position-dependent dust order | exact where flags are off |
So today an UNSAT means "no door in this palette whose run is race-free (no flag) within T". To
make it a proof every flag must become a nondeterministic choice (either order) and the residual
unflagged mismatches fixed.

## Log
* 23:30Z start; 00:05Z model + diff harness compiling; 00:20Z e89b re-found from its placement.
* 00:40Z fixed-pistons batch over families 7x8-006..033 killed (memory: 6 x 28 GB; and 008-017,
  013/014 are known dead); restarted on pistmodel's 22 live exact mechanisms (3 jobs).
* 00:57Z per-cell round: 2.3x fewer clauses for the whole box; fuzz identical to the per-piston
  round. Whole-box 7x8 find started (door at (2,2) and (2,3), T=36).
