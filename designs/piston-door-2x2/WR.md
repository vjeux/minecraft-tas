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

## Calibrations (designs/piston-door-2x2/selftest/)

- `classic.txt`: the classic with its lever on a block beside the frame. It must WORK
  (22 blocks, 7x4x1).
- `recessed.txt`: the same, with its opening declared one block in front of the door blocks
  (not flush). It must be refused ("not closed at rest").
