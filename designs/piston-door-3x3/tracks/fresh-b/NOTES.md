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
