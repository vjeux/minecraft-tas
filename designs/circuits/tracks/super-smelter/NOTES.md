# Super smelter track (designs/briefs/super-smelter.md)

Judge: the smelter judge's `chest` run (`mctas circuit smelter-run --run chest [--sim]`;
`--rotations 4` = 4 rotations x 4 positions in the game): a double chest of raw iron (3456) dumped
in the input chest, the fuel chest kept stocked with coal; the number is the minutes until the last
ingot, against the ceiling (3456 / 6N min).

| design | blocks | minutes | % ceiling | per furnace | coal kept | iron | rests |
|---|---|---|---|---|---|---|---|
| obvious-8 | 48 | 87.1 | 83% | 522 ... 166 | 5632 | 200 | yes |
| obvious-16 | 96 | 87.1 | 41% | furnaces 9-16 never smelt | 11264 | 400 | yes |
| **bs8** | 108 | **72.2** | **100%** | 432 each | 5344 | 240 | yes (no clock) |

## bs8: the self-clocked metered line (layout B)
- Each furnace: the input line hopper L, a LOCKED hopper D under it (takes one raw iron per window),
  an OPEN hopper E under D (pulls D's item at once and drains it into the furnace top), a locked side
  hopper Y (one coal per window from the fuel line T), the output hopper O.
- One dust network locks every D and Y (a dust line on the stones beside the D row, another on the
  stones beside the Y row), fed by a torch. The last line hopper faces a comparator instead of a
  hopper: when the line has filled up to it, the comparator (through a 4 gt repeater) turns the
  torch off for a few ticks and every D and Y acts once.
- The next window opens when the line has filled again (56 gt); when the chest is empty, nothing
  moves (no clock: it rests).

## What the mechanics forced (read in HopperBlockEntity, measured on redsim = game)
- A locked hopper neither pushes nor pulls. A locked D that has to push its own item into the
  furnace (no E under it) holds it until the next window: when the chest runs out the windows stop
  and ~290 raw iron per furnace stay locked in the D hoppers (first bs8 draft: jammed).
- Items move one hopper per 7 gt in a flowing line (`tryMoveInItem`: an item entering an empty hopper
  sets its cooldown to 8 minus the skipped tick) but enter it every 8 gt: one line hopper in eight is
  empty at any tick, and the empty one moves back one position per tick. A window that opens while
  hopper m is empty starves position m for good: D(m-1) ticks before L(m-1) (the block map is
  ordered x, y, z: a hopper below ticks first), takes the item that would refill L(m), and the line
  upstream is drained by the same window. With the window 4 gt after the last hopper fills,
  position 3 never got an item (7 furnaces did the work: 82.5 min); 8 gt after (the 4 gt repeater),
  the empty hopper is the last one itself, which holds its item: every furnace takes one per window.
- A clock instead of the comparator (b8, clk-rt-uloop-p142, 144 gt in the machine): its 12 gt
  window let the first furnace take 2 items a window, a period that is a multiple of 8 starved the
  same positions every window, and items not taken piled up in the last line hopper, which only
  the last furnace drains: 172.9 min.

## Open
- 16 furnaces: 18 line positions with no furnace where the empty hopper falls (a dust line reaches
  15 locks, so the D line needs a repeater, which delays the west half's window by 2 gt and moves
  its empty hopper). bs16b (positions 7 and 9 without furnaces): west half 276 each, positions
  10-16 every other window (138), 46.4 min: during the 8 gt delay a second item reaches the last
  hopper, the window stays open 8 gt longer and the east half's windows split. Next: a rising-edge
  pulse (subtract comparator) so a window is one action whatever the last hopper holds.
