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
