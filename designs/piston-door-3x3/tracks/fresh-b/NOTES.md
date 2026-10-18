# fresh-b: fresh 3x3 design, minimum block count

## Analysis (first principles)
- Push limit 12 blocks per piston action: no big slime frames (left col + backbone = 6 max sensible).
- Slime can touch door blocks only from the ring around the door in plane z=0 (corridor must be clear).
- C (center) never touches a non-door cell: it can only be pushed by a door block that moves toward it
  (the pusher then sits in the opening) or pulled by a sticky head/slime entering a vacated edge cell.
  => C needs a 2-stage puller (A riding B) or pump + puller.
- "Pump" trick (push line, sticky retract pulls only the first block back) moves a line 1 cell, but
  contact is lost afterwards: it cannot give C two moves on its own.
- Wiring dominates v2 (94 of 115). Dust costs 2 blocks/cell (dust + floor); observers/pistons/torches
  are 1 block. Side pistons sit at x<=-2 and x>=4, so a lever signal must reach both sides.

## Block budget (target)
| part | blocks |
|---|---|
| door | 9 |
| side backbones (slime) | 6 |
| pistons (L, R, T, A, B, P_B) | 6 |
| side signal (lever, torch/inverter, crossing) | ~15 |
| dance control (B window, A pulses) | ~15 |
| total target | ~50-60 |

## Log
- 05:31Z "TOP PUMP" mechanism validated with ideal inputs (pump-mech.txt + pump-drive.txt):
  top row glued to a 3-slime row (y=3); T (sticky, (1,4,0), facing down, RETRACTED) pumps:
  extend pushes slime row + top row + C down 1, sticky retract pulls slime+top row back, C stays at
  (1,0,0). A (bottom, facing up) pulls BM down, P_B pulls BM away in z (y=-1), A extend+retract pulls
  C to (1,-1,0). U (sticky at (0,5,0), head on slime (0,3,0)) lifts slime row + top row + T.
  Close: U+sides back, A 1-tick spit (C up 1), P_B returns BM, A extends (BM+C up). Works.
- 06:05Z mech3 (mech3.txt/.drive): top pump + PUSH-open sides (LP (1,-1,-1) west, RP (2,-1,0) east,
  one block P (2,-1,-1) powers both) + the LEFT structure carries BM sideways at y=-1 (carrier slime
  (0,-1,0), needs connector (-1,-1,0) or the backbone is left behind on the return pull) + A is
  pulse-only (retracted at rest). Validated with ideal inputs. All signals are the SAME order on open
  and close: A pulse (lever observer) -> sides (L8) -> T pump (>=4gt) -> A pulse (~L14) -> U lift (NOT L18).
  Mechanism = 25 blocks (9 door, 11 slime, 5 pistons): too heavy to beat the 48-51 record.
- 06:20Z slime-free sf1 (sf1-mech.txt/.drive): 6 side pullers, T pumps the middle column (TM ram, BM
  and C down 1), P_B takes BM aside, A pulses grab C, B lifts T, T grabs TM. Validated with ideal
  inputs (opens t=28, closes t=54). Wiring blocker: any source that powers T's upper spot (1,4,0)
  also quasi-powers T in its lower spot (QC cell = (1,4,0)), so the open-time grab pulse also fires
  on close after B pushed T down and pumps the closed column. Needs asymmetric B (NOT(L18 OR L26)
  via a locked repeater) or an open-only pulse. Also: an observer in a piston's QC cell watching
  that piston, outputting upward, makes it oscillate (seen with RP2).
- 06:57Z **pd3 WORKS: 61 blocks, solid** (`door gen pd3`, designs_fresh_b.rs::pd3; BEST.txt; archived).
  Fully pulse-driven top-pump door (mech3p mechanism: U also pulse-only, retracted with a gap).
  Ideal timing measured with `door ideal mech3p.sched` (tolerances: s in a1+5..9, t in s+1..a2-1,
  tl 3..7, a2 >= t+1, u >= t+tl+1). Control: lever + observer O_A1 on it (A pulse 1), dust ->
  R6 (L6) watched by O_S (pulse into P = both side pushers), R6 -> block -> dust -> repeater (L14)
  watched by O_A2 (A pulse 2); a torch tower on the lever block (2 gt per stage) up the front:
  O_T watches the L@8 torch -> 4gt stretch repeater -> obsidian -> T (lower spot only); the NOT L@10
  torch -> dust -> repeater (L18) watched by O_U -> U. Opens and closes in 22 ticks.

## pd3 block budget (61)
| part | blocks |
|---|---|
| door | 9 |
| slime: left structure 5 (backbone 2, BM carrier, connector, push cell), right 3, top row 3 | 11 |
| pistons A, LP, RP, T, U (all retracted at rest) | 5 |
| bottom control: lever + block, O_A1, 2 dust + 2 floors, R6 + floor, O_S, P, block, dust, R14 + floor, O_A2 | 16 |
| top control: torch tower 9 (W0 + 4 blocks + 4 torches), O_T, R_T + floor, obsidian, 2 dust + 2 floors, R18 + floor, O_U | 20 |
`door minimize --moves all` finds nothing (61 is a local minimum for removals/pairs/moves/reroutes).

## Why the top pump does not beat 47
* Its mechanism is as heavy as rb6's (25 vs 26) but its action is split: the pump (T) and the lift
  (U) sit above the door while A sits below it, so the control pays ~20 blocks to climb from the
  lever to y=4..6. rb6 keeps every piston but the bottom-middle at the top and triggers that one
  locally from a moving structure.
* A needs an EARLY pulse (pull BM down before the left structure drags it aside), so the lever (or
  something that changes at the flip) must be next to A; the late pulses (T pump, U lift, A grab)
  could come from moving parts, but nothing moves near the top before the pump itself.

## Findings worth reusing
* Push-open sides from the doorway floor with ONE block (2,-1,-1) feeding both pushers LP (1,-1,-1)
  west and RP (2,-1,0) east; with a 2gt pulse they push+spit on open and pull back on close.
* A side structure can carry BM away at y=-1 (carrier slime next to the bottom-middle column cell)
  and push it back on close; the carrier needs a slime connector to the backbone, otherwise the
  backbone stays behind on the return pull (a pushed slime line has no glue on the way back).
* The pump: push a line, sticky retract pulls only the first (glued) block back. Moves C one cell
  without any piston touching C.
* Pitfalls: an observer in a piston's quasi cell watching that piston, outputting upward, makes it
  oscillate (its output block quasi-powers the piston); a torch tower stage next to a side piston's
  quasi cell re-extends it; any source that powers a piston's upper spot quasi-powers it in the spot
  below (sf1's T riding B) unless that lower piston gets no block update (BUD-safe if the source
  powers a block next to the upper spot instead of the spot itself).

## Next ideas
* pd3: replace the 9-block torch tower with an observer chain (1 block / 2gt, no floors) climbing
  the front; derive U's late pulse from that chain instead of a repeater line (~-4 blocks).
* A mirrored pump (C exits UP, bottom row glued to a slime row and pumped from below, A grabs C at
  the top, the right structure drags TM aside at y=3 like rb6) would put A, TM's exit and the side
  pushers at the top; only the pump and the bottom-row lowering stay below.
* sf1 (slime-free top pump) with the BUD-safe upper-spot source above is buildable now; it needs ~6
  side pullers so it will not beat 53 either.

## 07:20Z pd3 61 -> 53 (pd3b 59, pd3c 54, pd3d 53 = BEST.txt), all verified at 32 positions
Patches (apply with `door patch`): pd3b-from-pd3.patch, pd3c-from-pd3b.patch, pd3d-from-pd3c.patch.
* Every lever-to-piston delay is now an OBSERVER CHAIN (1 block per 2 gt, no floors, the pulse stays
  2 gt) instead of a repeater/dust level line (2 blocks per cell): sides = O_A1 -> O2 -> O3 -> O_S;
  A's second pulse = O_S -> 4 observers -> observer under A; U = O_T -> 4 observers -> observer whose
  back points into the cell ABOVE U (quasi-power: one observer less than reaching U's side).
* Chain lengths are parity-bound (Manhattan distance between the source and the target cell), and
  every event lands on an even tick, so "one tick too early" costs two observers, not one.
* `door minimize --moves all --plateau` finds nothing on pd3 / pd3c / pd3d.
* pd3d control (28): lever + block, O_A1, O2, O3, O_S, P, 4 + 1 observers for A's 2nd pulse,
  torch tower 7 (to y=4), O_T + 4gt repeater + floor + block next to T, 4 + 1 observers for U.

## 08:15Z MIRRORED PUMP: mp6 46 -> mp7 45 -> mp8 43 (= BEST.txt), opens 16 / closes 16
Files: `door gen mp6` (designs_fresh_b.rs::mp6), patches mp7-from-mp6.patch, mp8-from-mp7.patch;
mechanism-only checks: mp-mech.txt + mp.sched / mp2.sched (first order), mp5-mech.txt + mp5.sched
(final order). All verified `--extra-offsets 28`.

### Mechanism (25 = door 9 + slime 11 + 5 pistons, all retracted at rest)
* Only BM and the bottom-LEFT corner ride a 2-slime row (0,-1,0),(1,-1,0); the right column is 3 high.
* Pump P (0,-2,0, under the row): one 4gt pulse pushes the row up (BM pushes C into TM's cell) and
  pulls it back (C is not glued, it stays at (1,2,0)).
* U (1,-3,0, gap (1,-2,0)): one 2gt pulse at the end pulls the row down (BM + corner out); P is
  pushed down with it and dragged back up on close (a retracted piston glued under slime RIDES it).
* A (1,4,0, gap (1,3,0)): pulse 1 pulls TM up into (1,3,0); pulse 2 pulls C up into (1,3,0).
* Left structure (slime (-1,1..3,0)) pushed west by H (0,3,0); right structure (slime (3,0..4,0)
  + carrier (2,3,0)) pushed east by F (2,4,0); the carrier drags TM from (1,3,0) to (2,3,0).
* ORDER (same on both edges): A1, H | pump | F | A2 | U. The pump only needs TM up and the LEFT
  side open, so it runs before F; then A2 can be fired by F's own motion.
  Ideal windows (mp5.sched): h 0..6, a1 <= t-3, t 5..9, tl 3..7, s 5..8 (and >= a1+5), a2 >= s+2
  and > t, u >= t+tl+1; a double U pulse breaks the close.
  Close: A1 pushes C from (1,3,0) down to (1,2,0) (spit), H pulls the left side back, the pump is a
  no-op (P sits one lower), F brings TM back into (1,3,0), A2 pushes TM + C home (spit), U lifts the
  row (spit; P rides back up).

### Control (18): FLAGS = plain blocks glued to a moving slime structure, watched by an observer
that touches no slime (an observer next to slime would be dragged). Watching the cell a flag moves
INTO (instead of the one it leaves) is as good and frees the observer's back side.
* Lever (0,3,-1) on the ceiling block S' (0,4,-1): powers H directly; S' feeds R_F (1,4,-1, d4,
  floor (1,3,-1)) -> block (2,4,-1) -> F at 8.
* Left structure moves at 1: top-left flag (-1,4,0) -> O (0,4,0) -> A (A1 at 3); bottom-left flag
  (-1,0,0) -> O (-2,-1,0) watching (-2,0,0) -> block (-2,-2,0) -> R_P (-1,-2,0, d2 = 4gt pulse) -> P.
* Right structure moves at 8: top-right flag (3,5,0) -> O_a (2,5,0) -> A's quasi cell (1,5,0) (A2 at
  10); bottom-right flag (3,-1,0) -> O (3,-2,0) -> O (3,-3,0) -> O (2,-3,0) -> U (at 14).

### 08:30Z mp8 = 43 blocks (25 mechanism + 18 control), opens 16 / closes 16, verified at 104 positions
Control blocks: lever + S' (2), A1 = top-left flag + observer (2), F = repeater + floor + block (3),
A2 = top-right flag + observer (2), pump = bottom-left flag + observer + block + d2 repeater + floor
(5), U = bottom-right flag + 3 observers (4). `door minimize --moves all --plateau --plateau-pairs`
and a 400-step neutral walk find nothing.
Why each piece is still there:
* U needs ONE 2 gt pulse after the pump ends (u >= t+tl+1 = 12); the cells next to U that are not
  next to the lowered slime row are (2,-3,0), (1,-4,0), (1,-3,+-1), and the nearest late event is
  the right structure's bottom flag at (3,-1,0): 3 observers is the shortest legal chain. An observer
  watching the pump repeater or P itself would give two pulses (the second one breaks the close).
* The pump needs >= 3 gt (a 2 gt pulse spits the row up and leaves it there), so a repeater; the
  observer that sees the flag can only sit under the flag's destination, so its output block Y is
  needed to feed that repeater from behind.
* A1 cannot come from an observer on the lever while the lever also powers H directly (no cell is
  next to the lever and has its back on A), hence the flag on the left structure.
What fails: mp-mech order (pump after F, full slime row) needs A2 after the pump at the top and U at
the bottom: ~23 control blocks. The reorder (only BM + the bottom-left corner ride the row, pump
before F) is what lets F's own motion fire A2 with one observer.

## Final report (08:40Z)
* Best: **43 blocks, solid** (also the fastest solid door: opens 16, closes 16), `BEST.txt` = `mp8.txt`
  on explore/fresh-b (built by `door gen mp6` + mp7-from-mp6.patch + mp8-from-mp7.patch); verified
  `--extra-offsets 100 --cycles 5`.
* Mechanism: mirrored top pump. A pulls TM up, the right structure's carrier drags it east; a pump
  under a 2-slime row (BM + bottom-left corner) pushes C up into TM's cell and pulls the row back;
  A pulls C up; U lowers the row (the pump piston rides it). Sides pushed from above (H west, F east).
  All pistons retracted at rest, same order on both lever edges, fired by "flags" (plain blocks on
  the moving side structures, watched by observers that touch no slime).
* Path: top pump pd3 61 -> 53 (observer chains), mirrored pump mp6 46 -> 43.
* Failed: top pump (pd3) can't go below ~50 (top/bottom/top/bottom signal order); slime-free sf1
  (a piston's upper-spot source quasi-powers it in its lower spot); mirrored pump in the first order
  (~48: needs a late A2 at the top). 43 is a local minimum (minimize, plateau, pairs, walk).
* Gadgets: flag-observer, slime-rider, pump.
* Next ideas: a U trigger closer than 3 observers (U is 3 cells from the nearest late mover); flags
  for fc2-style designs (any piston that only needs to fire after a structure has moved costs one
  flag + one observer).

## 08:55Z flags on fc2 (fresh-c, 40) and fc2-style control on the mirrored pump: no gain found
* fc2's P_M already fires off a moving door block (observer + block = 2); a 1-observer version needs
  an extra slime + flag (3). Its B window (observer, block, d4 repeater, dance observer = 4) off a
  right-structure flag costs flag + observer + d4 repeater (+ floor) + dance observer = 4-5. P_L's two
  observers can't be replaced: nothing moves before it, and a steady lever-driven push piston must
  sit at x=0, where X's dust support is.
* RB shuttle on the mirrored pump (A extended at rest, Q moves the RB to F): Q + RB + d2 repeater +
  floor (4) replace F's 3-block line: +1.
* mp8's U chain: an observer at (2,-2,0) firing U through its quasi cell (1,-2,0) works electrically,
  but the lowered row's slime lands in (1,-2,0) and drags that observer up on close. Every other cell
  next to U is 2+ cells from the nearest late mover, so 3 observers stay.
* mp8's pump path: the pump needs >= 3 gt (2 gt spits the row up), so a repeater is required, and the
  only observer cell that sees a left-structure flag without touching slime is under the flag's
  destination, whose output cell is not a repeater input: 5 blocks stay.
