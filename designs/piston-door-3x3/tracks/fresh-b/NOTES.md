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
