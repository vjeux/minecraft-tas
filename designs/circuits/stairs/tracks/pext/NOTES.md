# Piston staircases (track pext, 2026-10-21)

The kind: `circuit: piston-stairs N` (crate::bridge, docs/circuits.md "piston-stairs N"). A lever
extends a walkable staircase of N steps up out of a wall and takes it fully away again. The judge
reserves the STEPS (step k: k along `travel` and k up from `port stairs`), the LANDINGS (the floor
block before the first step and the upper floor's block after the last, full cubes), the PASSAGE
(3 cells above every step and the lower landing: a walker jumps up each rise; 2 above the upper
landing) and the HIDDEN cells under the steps down to the floor level (empty at rest, anything when
out). Extended: every step a full cube, nothing moving, the passage empty; then the real-use lever
flips (`flips`).

## Records (`archive/`, every one game-robust, flips safe, out 3 gt, back 3 gt)
| N | design | blocks | box | volume |
|---|---|---|---|---|
| 3 | stairs-stone-3 | 11 | 4x4x5 | 80 |
| 4 | stairs-stone-4 | 14 | 4x5x6 | 120 |
| 5 | stairs-stone-5 | 18 | 4x6x7 | 168 |
| 3 | stairs-slime-3 | 8 | 3x4x5 | 60 |
| 4 | stairs-slime-4 | 10 | 3x5x6 | 90 |
| 5 | stairs-slime-5 | 12 | 3x6x7 | 126 |

* Slime: the N steps are slime blocks in the side wall, joined by one slime block under each step
  from the second on (diagonal blocks only touch at an edge), one sticky piston behind, an obsidian
  upper landing (the top step would drag any movable block there into the wall).
* Stone: the stone steps in the wall face, the same slime piece behind them (each stone needs a
  sticky block behind it to come back: stone does not stick to stone), one sticky piston behind.
  N = 5 moves 14 blocks, over a piston's 12: the piece splits in two where no connector is needed
  (under the fourth step), each part with its own piston; the lever stands on a stone block touching
  both pistons.

## Why these are the minimum under the judge's rules
* Length N + 2 and height N + 1: the box holds both landings (the lower one below the floor level
  and one back, the upper one after the last step).
* Every step is empty at rest and its headroom and the cells under it too, so each step is stored
  outside the staircase's plane and moves into it. From the side it moves 1 cell (from below, step k
  would travel k + 1 cells: an extender), and a piston pushes only along its facing, so the pusher
  stands behind the stored piece: 3 layers for slime steps (staircase, steps, piston), 4 for stone
  (staircase, stones, sticky blocks, piston).
* Blocks: one pusher moves one connected sticky piece; N diagonal steps (or the N sticky blocks
  behind stone steps) need N - 1 connectors: slime 2N + 2 (N steps, N - 1, a piston, 2 landings),
  stone 3N + 2 (N stones more); a piston per step instead needs N pistons and their power. Stone at
  N = 5: 3N + 3 (a second piston and the lever's block).
