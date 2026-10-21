# Track macrosat: an independent bounded check of small flat doors (clockinfra, 2026-10-20)

Coordinator's brief (00:45Z): an exhaustive check of w86's claim that a 7x7 (49) flush 3x3 door
cannot be wired, built WITHOUT c70b's generator assumptions; output a counterexample mechanism
with feedable firings (for pockets3 / pext to wire) or a bounded proof with its exact
assumptions; then 6x8 (48).

Tool: `mctas door macrosat` (crates/piston-door/src/macrosat.rs). Bounded model checking with
CaDiCaL: the box is a grid of one-hot cells at every macro step, the layout is FREE (or fixed
with `--fix design`), the solver picks the layout and the actions together. No providers, no
families, no catalog: anything the bounds allow is in the search. `macrosat sweep --boxes
"x0,y0,x1,y1;..." --hs 6,8 --jobs N --out DIR` runs a grid in threads; `--export PREFIX` writes a
SAT model as an ideal-input design + plan (`door ideal-rs` replays it).

## The model

* Cells (per step): air, door stone (movable conductor), obsidian (immovable conductor), redstone
  block (movable power source), a WIRING cell (a static component: nothing may push into it or
  move it; never changes), normal / sticky pistons in the 4 in-plane facings, retracted or
  extended, and their heads. Everything is in the door's plane (1 wide); nothing in front of or
  behind it by construction.
* A step fires a set of INDEPENDENT pistons: no two write one cell and none reads a cell another
  one writes, so every order gives the same result (a step = a sequential run in any order;
  `--seq` = one action per step). Actions that change nothing are left out. Actions:
  * retracted piston: a PUSH of the straight line in front (movable: stone, redstone blocks,
    retracted pistons; <= 12; must end on an air cell inside the box; obsidian, wiring,
    extended pistons and heads stop it) as EXTEND (stays out, head in front), PULSE (back at once:
    the line stays pushed, a sticky piston spits) or PULSE-PULL (sticky out and back: the head
    pulls the first block back; into air it pulls the block 2 ahead: one firing, a 4 gt pulse);
  * extended piston: RETRACT (head gone; sticky pulls the movable block in front of its head).
* Door: the 3x3 at (0..2, 0..2) is stone at rest; after `--h` steps it is air and the 12 frame
  cells are stone (FULL frame); after `--h` more the whole box equals the rest state.
* Mirror symmetry breaking (lex-leader on the rest layout) for boxes symmetric about x = 1.
* Feed (`--feed`), checked for every action in the state it fires from (the cells it uses not
  written by another action of the step):
  * a wiring cell (a controllable source) next to the piston on a non-front face, or next to the
    cell above it (QC); a conductor (stone, obsidian) in such a cell with a wiring cell or the
    lever next to it (strongly powered; a redstone block does not power blocks);
  * the single lever (`--lever`) next to the piston or to the cell above it: hung on a block that
    never moves, on its front/back face (rules.md: still 1 wide) or at the box edge on its outer
    face; above the box it also QC-feeds the top-row pistons beside it (e78k's lever on (1,4)
    holds (0,4) and (2,4));
  * a redstone block is a CONSTANT source: it fires a piston only by arriving in a feed cell (an
    extend) or leaving it (a retract), never at the first step of an edge (each edge starts
    settled); while one sits in a feed cell the piston cannot retract;
  * ISLAND START (coordinator 01:57Z, from clk-misc / pockets3 / d67's `door feeds`
    cannot_start): wiring regions (4-connected wiring cells, joined through one conductor
    between two of them) act only once started: next to the lever's block, or next to a cell
    that changed in an EARLIER step of this edge (an observer's delay). On with `--feed`,
    `--no-start` off;
  * `--private` (a candidate filter, not a proof rule): a wiring cell feeds one piston cell only.
* Still optimistic (an UNSAT is a proof within the model): no update / BUD requirement, no
  timing inside a step, misfires and holds ignored (QC coupling of stacked pistons!), rest
  stability, wiring cells can output anything. Not encoded yet: "a region that sees the same
  changes on both edges acts the same on both".

What the model does NOT cover (the assumptions of any UNSAT): actions interacting within one
tick (two block events of one round meeting, a pull aimed at a block still moving, a retract
while the line is moving: the "macro vs redsim" gaps c70b lists), moving observers (a moving
redstone block is the stronger stand-in), slime / honey, gravity blocks, entities (the rules ban
them for this board), pistons facing out of the plane.

## Calibration

| case | result |
|---|---|
| c77 w1 (7x7) with its plan forced (`--plan`, 13+13 firings) | SAT at once; with `--feed`: UNSAT at once (P (1,-1) unfed, as w86 / c70b say) |
| w1's layout, free plan | SAT (40 s, 8 parallel steps per edge; 666 s for 16 sequential steps); with `--feed --lever`: UNSAT (55 s, h 8) |
| e78k-h5-41b (the 56b record, a real WIRED 7x8 door), layout fixed | SAT with `--feed --lever` (h 10), also with `--private` and island start: the feed rules accept a real wired door. It caught two bugs (the lever above the box QC-feeding top-row pistons; feed cells over-restricted by the independence rule) |
| c70b's mm01-7x7 (works in redsim; 30 pulses on the open edge) | UNSAT at h 10, SAT at h 20 (502 s): long mechanisms need long horizons |
| w86 s1 (6x8) | SAT without and with the (pre-start) feed (h 10): w86's argument (one shared feed cell for two pistons acting at different times) is about timing, which the feed rules do not model |

## Results (devvm53196, ~/macrosat)

| run | result |
|---|---|
| 6x6 (both placements), no feed, h 8 | UNSAT in 66 s each: no 6x6 macro mechanism within 8 parallel steps per edge |
| 7x7 (all 6 placements: x0 -2 / -3 (the mirror of -1), y0 -3 / -2 / -1), no feed, h 8 | SAT in all 6 (23-54 min) |
| 7x7, literal feed (any free cell / QC path; before the redstone-block and start rules) | SAT within 6 steps in (-2,-2), (-2,-3), (-3,-2), (-2,-1); UNSAT (-3,-3) and (-3,-1) at h 6 |
| 7x7, + `--private` | SAT (-2,-2) h 6 (355 s): carries a piston into the square at (0,0), fed through the stone frame cell below it (a wiring cell at (0,-2) powers it); UNSAT (-3,-3) h 6 / h 8, (-3,-1) h 6. Exported as cands/m7-a: WORKS with ideal inputs in redsim (32 positions) and the real game (4 positions), open 56 / close 54, but its layout fails island start (UNSAT at h 6 / 8): u0-like |
| 7x7, + island start (with and without `--private`) | UNSAT: (-3..3, -3..3) and (-3..3, -1..5) at h 6 and h 8 (both); (-2..4, -3..3) at h 6 (both); (-3..3, -2..4) and (-2..4, -1..5) at h 6 (private) |
| 48 (6x8: x -1..4 (the mirror of -2..3) x y -4..-1 offsets; 8x6: x -4 / -3 (mirrors of -1 / -2) x y -2 / -1), feed + island start | SAT 8x6 (-3..4, -1..4) at h 6 (1138 s): cands/m86-a, WORKS with ideal inputs in redsim (32 positions) and the real game (4 positions), open 56 / close 54 — but NOT wirable (w86, pockets3, clk-misc): its lever on (2,4) holds SD(2,4) all edge (the plan pulses it), the L1 piston's "conductor powered from (-1,4)" has no component that can do it in this plane, one obsidian feeds 4 pistons at once, one cell must make 8 distinct ticks, and its ideal input behind (-3,0) QC-fires the piston below out of the box (9x6 as exported). Retracted; its lessons became the rules below. UNSAT 6x8 (-1..4, -1..6) and (-1..4, -4..3), 8x6 (-4..3, -1..4) and (-4..3, -2..3) at h 6 / h 8 |
| w86 s1 (6x8) layout, feed + island start, h 10 | UNSAT (138 s); SAT before the start rule: island start sees w86's argument |

## Wirability rules after m86-a (8e2cced03)

Hard (every run): the lever is a LEVEL (a polarity variable: on in the open edge or in the close
edge; a lever feed extends a piston while on and retracts it only at an edge's first step, when it
turns off; a piston the lever powers cannot retract or pulse in its on edge); a region starts from
the lever only when the lever's block is a conductor; a conductor is strongly powered only by what
this vertical plane allows (a horizontal repeater / comparator / observer whose input or watched
cell is in the box, a torch below it, dust on top of it).

`--strict` (w86 / pockets3 / clk-misc, filters, not proof rules): `--private`; CO-FIRING (a powered
conductor powers every piston next to it and QC-powers those whose above cell touches it; a wiring
cell's signal into a cell powers the piston there and QC-powers the one below: every retracted one
must act in the same step); the lever's pistons toggle only at an edge's first step and never move;
`--budget 2` (a wiring cell feeds firings in at most 2 steps per edge); `--static-cond` (conductor
feeds only through blocks that never move); `--frame3` (exactly 3 frame cells hold stone at rest).
The wired 7x8 record e78k passes `--strict` (h 10).

`--export` now places each ideal input where the chosen feed is: behind the piston (z = 1) for a
direct feed, behind its above cell for a QC feed plus a glass block placed and removed at z = -1
(the block update QC needs), a powered lever on a conductor's back face for a conductor feed.

| run | result |
|---|---|
| lever-level rule (no --strict; v6, then v8 with the corrected strong-power rule), 7x7 | UNSAT at h 6: (-2..4, -3..3), (-3..3, -2..4), (-2..4, -1..5); with the island-start runs: 5 of the 6 placements UNSAT at h 6, (-3..3, -3..3) and (-3..3, -1..5) also at h 8; (-2..4, -2..4) still solving |
| lever-level rule, 6x8 (-1..4, -2..5) | SAT at h 6 and h 8, but its layout is UNSAT under `--strict` / `--physics` (h 8) and its replay breaks: not physical |
| `--strict`, 48 | UNSAT 6x8 (-1..4, -1..6), (-1..4, -4..3), (-1..4, -3..4), (-1..4, -2..5), 8x6 (-4..3, -1..4), (-4..3, -2..3), (-3..4, -2..3), (-3..4, -1..4) at h 6; most also at h 8; h 10 / h 12 on clk-misc's box (2 UNSAT so far) |
| `--strict`, 7x7 | UNSAT at h 6 in every placement but (-2..4, -2..4); (-3..3, -3..3), (-3..3, -1..5), (-2..4, -3..3) also at h 8 |
| `--physics --max-pistons 14` (c70b's seeds) | UNSAT everywhere at h 6 / h 8 so far |

**What the bounds mean.** Every UNSAT is bounded by H parallel steps per edge. c70b's working
(unwired) mm01-7x7 needs 20 steps in the old permissive feed model (UNSAT at h 10, SAT at h 20),
so these results only rule out SHORT fed mechanisms; they are no proof that 7x7 cannot be wired. A
real proof needs much longer horizons (h 10 takes hours per placement) or an unbounded method.
They also cover only the macro palette: air, stone, obsidian, redstone blocks, static wiring cells,
normal and sticky pistons. Designs that carry an observer (it fires on arrival and powers what it
faces at its new cell) or ride signals on slime structures (fly's gadgets 2 and 3, explore/fly
tracks/fly/wip/) are outside it. A lever on a piston outside the box (holding it and, by QC, the
piston below) and QC through an empty above cell are inside it.

**The central 7x7 near-miss (2026-10-20).** The plain feed model found (-2..4, -2..4) SAT at h 6
(20 000 s; sat/v8/). On its fixed layout `--physics` was SAT at h 8, but the ideal-rs replay of
the export broke twice, and each break was a rule the physics filter lacked (now in `--physics` and
`--strict`):
- HOLD: a rising signal (an extend or pulse feed) also holds every extended piston it reaches,
  directly or by QC; such a piston cannot retract in that step (a QC-held one does not even get
  an update when the signal ends). The replay: (4,0) stayed out under the lever-pulsed conductor
  (4,2). Falling signals no longer force retracted neighbours to fire.
- REDSTONE-BLOCK STABILITY: a retracted piston next to a redstone block (any side but its front)
  extends at once, so it extends in the next step and never rests so. The replay: (4,-2) pushed
  its redstone block back next to (1,-2), which re-extended into the doorway.
- QC ARMING (w86): a retracted piston QC-armed by a redstone block next to its above cell, or by
  an extended piston right above it whose DIRECT holding source (a redstone block or a wiring cell)
  touches its cell, fires in the next step, at every settled state (a lingering arm fires on any
  later update). A QC-held piston arms nothing (a piston is no power source).
With them that layout is UNSAT up to h 12; the wired 7x8 e78k still passes `--physics` and
`--strict` (h 10). The redstone-block rule has no pushability exception (a powered piston whose
push is blocked by obsidian or an extended piston stays retracted in the game), so `--physics`
UNSATs stay filter results, as before.

**c70b's mm01-7x7 (works in redsim, unwired) on its fixed layout (2026-10-20).** With the current
feed rules (only real strong-power sources into a conductor) it has no fed schedule at h 20 even
without the lever and without island start (UNSAT, 716 s; the old permissive feed model found one
at h 20 in 502 s); with `--lever` (h 20, with and without island start) and with `--physics`
(h 20 and h 28) it is UNSAT too. So the best known 7x7 mechanism cannot be fed within 20-28
parallel steps in this model: its firings need feeds that no real source in the box gives.

**u2-T (clk-item's u2-7x7 with a real lever and torch, explore/tff tracks/clk-item/wip/) on its
fixed layout (2026-10-21, for w86).** Box -2..4 x -2..4, lever spot free, wiring free in the empty
cells, h 16 per edge: mechanics alone SAT (566 s); feed + lever without island start SAT (838 s);
with island start UNSAT (597 s); plus a movable observer allowed in any empty cell UNSAT
(1568 s); plus `--physics` UNSAT (1041 s). Every firing can be fed, but no lever spot, wiring or
observer placement starts every island before its first firing: w86's start-order argument
(stacks only cross downward; the top-left and top-right islands share no crossing), confirmed
within 16 steps. Not covered: an observer replacing one of its blocks, added redstone blocks.

Replaying a SAT model with ideal inputs (`--export`, `door ideal-rs`): 6 gt per step was too
short (a pull met a block still moving); exports now use 10 gt per step with one `let` per step
(retime with `--set`). Inputs at z = 1 QC-couple a piston to the one below it (w1's (-2,1) /
(-2,2)): such a replay can misfire where the model fires one piston alone.
