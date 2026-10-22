# Piston bridges (track pext, 2026-10-21)

The kind: `circuit: piston-bridge N` (crate::bridge, docs/circuits.md "piston-bridge N"). A lever
extends a walkable bridge across a gap of N blocks and takes it fully out of the gap again. The
judge reserves the WALKWAY (N cells from `port gap` along `travel`), the BANKS (one design block at
each end, a full cube: the ground), the PASSAGE (2 cells above walkway and banks) and the PIT (every
cell under the walkway). At rest the walkway, passage and pit are empty; extended, every walkway cell
is a full cube (a walker crosses on one level), nothing moving, passage and pit empty; the rest state
comes back exactly after the off edge; then the lever flips of the real-use audit (`flips`).

## Records: the slime row (`archive/bridge-slime-N`, N = 3..8, every one game-robust, flips safe)
| N | blocks | box | volume | out / back |
|---|---|---|---|---|
| 3 | 6 | 3x1x5 | 15 | 3 / 3 gt |
| 4 | 7 | 3x1x6 | 18 | 3 / 3 gt |
| 5 | 8 | 3x1x7 | 21 | 3 / 3 gt |
| 6 | 9 | 3x1x8 | 24 | 3 / 3 gt |
| 7 | 10 | 3x1x9 | 27 | 3 / 3 gt |
| 8 | 11 | 3x1x10 | 30 | 3 / 3 gt |

N slime blocks in the side wall, one sticky piston behind the row, a lever on the piston's back,
obsidian banks. The row moves as one (slime sticks together; a piston moves up to 12 blocks, so the
same build works to N = 12). The banks must not stick to slime: with stone banks the row drags them
back (the selftest's `cal-bridge-drags-banks`).

## Why these are the minimum (volume and blocks), under the judge's rules
* Length N + 2: the box holds both banks.
* Blocks N + 3: N walkway blocks, 2 banks, at least one piston (the lever is the input, not
  counted).
* Cross-section 3 x 1: at rest the walkway, its passage and its pit are empty, so the N bridge
  blocks wait outside that column and move into it. Above the walkway the passage takes two cells,
  so blocks stored above would travel 3 cells (an extender); under it is the pit. Stored beside it
  (1 cell), they move sideways by 1, and a piston pushes only along its facing, so the pusher stands
  behind the stored row (the third column). Fed along the span from a bank instead, the row would
  travel N cells (an N-reach extender, far bigger).
* So 3 x 1 x (N + 2) = 3N + 6 and N + 3 blocks are optimal, and extend / retract 3 gt is a single
  piston's move.

## Not done (optional)
* A 2-wide bridge (`width: 2`): two mirrored slime rows (one per side) and one lever reaching both
  pistons around a bank; not tileable side by side (the storage stands beside the walkway).
* A walkway without slime (stone): one sticky piston per block, powered by a dust line on top of
  the pistons (3 x 2 x (N + 2)).
