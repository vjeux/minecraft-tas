# The 2x2 piston door (`door-size: 2`)

The most-built door (coordinator 2026-10-21). Judge: `mctas door verify <design> --extra-offsets 28`
on a design with the header `door-size: 2`. It applies the 3x3 door's rules to a 2x2 opening:
flush when closed, a clear 2x2 opening and walkway when open, lever input, nothing in front or
behind, 3 cycles at 32 positions, rotation-safe boards
([docs/rules.md](../../docs/rules.md#the-2x2-door-door-size-2)). Size is the volume, the bounding
box of the build over a whole cycle with the lever excluded (`door properties`), as for the 3x3.

## Boards

| board | leader | blocks | size (volume) | open / close ticks | notes |
|---|---|---|---|---|---|
| Smallest (flush) | **classic6** (archive/classic6-20b.txt) | 20 | 6x4x1 (24) | 1 / 3 | solid, slime-free, flat, rotation-safe |
| Fastest | **classic6** | 20 | 6x4x1 (24) | 1 / 3 | |
| Slime-free | **classic6** | 20 | 6x4x1 (24) | 1 / 3 | |

## Captions

- **classic6** (clk-item): Four sticky pistons, two on each side of the opening, each hold one
  door block in the opening while they are extended. The lever sits on the frame above the
  door and powers the frame's blocks, partly through a line of dust. Each powered block works
  the piston below it directly and the bottom piston through quasi-connectivity (a piston also
  turns on from power reaching the cell above it). Flip the lever off and the pistons pull the
  blocks into the wall.

## What else was tried, and why 24 is probably the floor

- **Why the volume can't go lower.** A door block can only move within the door plane, since
  the cells in front of and behind it are walkway. A sticky piston sits two cells from the block
  it pulls. So the four pistons span 6x2 (all sideways), 4x6 (one column sideways, the other up
  and down) or 2x6 (top row up, bottom row down). The lever's power must then reach both sides:
  - across the opening, on a row of dust over supporting blocks: two more rows, so 6x4 = 24;
  - or up and down the side, where one column has no way to carry power vertically and a dust
    staircase needs a second column or a second layer.
  Designs with slime and fewer pistons are wider still: a slime block must sit between the head
  and the door block.
- **Dust directly on the top pistons** (18 blocks, the same 6x4x1): it pops. An extended
  piston's top face isn't full, so dust can't stand on it (`door drive`: the bottom pistons are
  left unpowered and retract). The frame block under the dust is needed.
- **Fastest:** opening takes 1 tick, the minimum (the block becomes a moving block at once).
  Closing takes 3: a piston extends over 2 ticks, after the tick its power arrives.

## Calibrations (designs/piston-door-2x2/selftest/)

- `classic.txt`: the classic with its lever on a block beside the frame. It must WORK
  (22 blocks, 7x4x1).
- `recessed.txt`: the same, with its opening declared one block in front of the door blocks
  (not flush). It must be refused ("not closed at rest").
