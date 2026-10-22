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
* 2-wide for gaps 7 and 8 (each side needs two pistons around an obsidian block, and the dust link
  must reach both blocks: not cheap).

## Stone walkway (`archive/bridge-stone-N`, N = 3..8, every one game-robust, flips safe)
A slime walkway slows a walker and bounces a fall; these walk like normal ground.
| N | blocks | box | volume | out / back |
|---|---|---|---|---|
| 3 | 9 | 4x1x5 | 20 | 3 / 3 gt |
| 4 | 11 | 4x1x6 | 24 | 3 / 3 gt |
| 5 | 13 | 4x1x7 | 28 | 3 / 3 gt |
| 6 | 15 | 4x1x8 | 32 | 3 / 3 gt |
| 7 | 19 | 4x1x9 | 36 | 3 / 3 gt |
| 8 | 21 | 4x1x10 | 40 | 3 / 3 gt |
A stone row in the wall face, a slime row behind it, one sticky piston behind that (2N + 3
blocks). Coming back, the slime pulls each stone home: stone does not stick to stone, so every
stone needs its own sticky block behind it, and that row needs a pusher behind it: 4 columns
(walkway, stone, sticky, piston) in one layer. The other way, one sticky piston per stone, needs
its power on a dust line on top of the pistons (3x2, 6N + 12 > 4N + 8). A piston moves at most
12 blocks (N stone + N slime), so gaps 7 and 8 split the sticky row into slime then honey (they do
not stick to each other), each with its own piston, the two pistons on either side of an obsidian
block that carries the lever (2N + 5 blocks; obsidian because slime drags any movable block).

## 2-wide, stone walkway (`archive/bridge-stone-2wide-N`, N = 3..6, game-robust, flips safe)
| N | blocks | box | volume |
|---|---|---|---|
| 3 | 38 | 8x2x6 | 96 |
| 4 | 42 | 8x2x7 | 112 |
| 5 | 46 | 8x2x8 | 128 |
| 6 | 50 | 8x2x9 | 144 |
The stone-walkway bridge mirrored (one row from each wall; the two stone rows meet in the middle
and do not stick). A slime walkway cannot do this: the two slime rows would stick together and one
piston would pull both. The one lever reaches both pistons through 10 redstone dust around the
near end, behind the bank (the passage above the banks and the pit under the walkway are
reserved), each dust on a block: 20 of the 4N + 26 blocks. The two pistons are at least 7 cells
apart (each pushes its row from behind), so no single powered block reaches both.
