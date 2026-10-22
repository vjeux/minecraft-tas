# The 1x2 piston door (`door-size: 1x2`)

One block wide, two tall: the most-built player entrance (coordinator 2026-10-21). Judge: `mctas
door verify <design> --extra-offsets 28` on a design with the header `door-size: 1x2`. It applies
the 3x3 door's rules to the 1x2 opening: flush when closed, a clear opening and walkway when open,
lever input, nothing in front or behind, 3 cycles at 32 positions, rotation-safe boards
([docs/rules.md](../../docs/rules.md#rectangular-doors-door-size-wxh-the-1x2-door)). Size is the
volume as the regulation counts it (`door properties`, as for the 3x3 and the 2x2): the bounding box
of the circuitry over a whole cycle, the door blocks, the frame and the lever excluded.

## Boards

| board | leader | blocks | size (volume) | open / close ticks | notes |
|---|---|---|---|---|---|
| Smallest (flush) | **stacked** (archive/stacked-5b.txt) | 5 | 2x2x1 (4) | 1 / 3 | solid, slime-free, flat, rotation-safe, seamless SUPER |
| Fastest | **stacked** | 5 | 2x2x1 (4) | 1 / 3 | |
| Slime-free | **stacked** | 5 | 2x2x1 (4) | 1 / 3 | |
| Seamless, full frame when open | **stacked-frame** (archive/stacked-frame-9b.txt) | 9 | 2x2x1 (4) | 1 / 3 | seamless SUPER, frame 6/6, rotation-safe |

Both lever on the back of the top piston (`lever_support=no` in `door properties`: a retracting
piston's base turns into a moving block for a tick under the lever; the lever stays, the judge's 32
positions pass).

## Captions

- **stacked** (pext): Two sticky pistons stacked beside the opening each hold one door block in
  the opening while they are extended. The lever hangs on the back of the top piston and powers it
  directly; the bottom piston turns on through quasi-connectivity (a piston also turns on from
  power reaching the cell above it). Flip the lever off and both pistons pull their blocks into the
  wall beside the opening.
- **stacked-frame** (pext): The smallest door with stone on the other three sides of the opening.
  Open, the two door blocks sit in the wall beside the opening, so all 6 blocks around the doorway
  are the door's stone and no piston or redstone shows from either side, closed or open.

## Why these are the floor

- **Volume 4.** A door block can only move within the door plane (the cells in front of and
  behind it are walkway), and a sticky piston sits two cells from the block it pulls: each of the
  two door blocks needs a piston and its head in the plane, 4 cells of circuitry at least. Stacked
  on one side they make a 2x2 box. One piston for both blocks needs a sticky block between the head
  and the door blocks (a door block is a plain block: it does not stick to its neighbour): a third
  column, 6 or more. Pulling the blocks up and down instead spans 1x6, and one column cannot carry
  the lever's power from one piston to the other.
- **The lever.** On the back of the top piston it powers that piston directly and the bottom one by
  quasi-connectivity: no block of wiring at all. A lever on a block of its own (the calibration
  below, `lever_support=yes`) puts that block in the circuitry: volume 6 (3x2x1).
- **Fastest.** Opening takes 1 tick, the minimum (the door blocks become moving blocks at once);
  closing takes 3: a piston extends over 2 ticks, after the tick its power arrives.
- **Seamless.** The door blocks retract into the frame beside the opening, so open, the frame there
  is the door's own stone, and closed, the piston heads sit in the door plane's ring, covered by
  the door blocks: SUPER (`door properties`). The full frame adds the other four frame cells (two
  beside, one above, one below) as plain stone, which is not circuitry: the volume stays 4.

## Calibrations (designs/piston-door-1x2/selftest/)

- `classic.txt`: the classic with its lever on a block behind the top piston. It must WORK
  (6 blocks, 5x2x1, rotation-safe).
- `recessed.txt`: the same, with its opening declared one block in front of the door blocks (not
  flush). It must be refused ("not closed at rest").
