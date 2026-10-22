# Super smelter track (designs/briefs/super-smelter.md)

Judge: the smelter judge's `chest` run (`mctas circuit smelter-run --run chest [--sim]`;
`--rotations 4` = 4 rotations x 4 positions in the game): a double chest of raw iron (3456) dumped
in the input chest, the fuel chest kept stocked with coal; the number is the minutes until the last
ingot, against the ceiling (3456 / 6N min). Fuel: the coal the load needs plus a stack (496), never refilled (coordinator 2026-10-21; with unlimited coal the obvious rows took 87.1 min).

| design | blocks | minutes | % ceiling | per furnace | coal kept | iron | rests |
|---|---|---|---|---|---|---|---|
| obvious-8 | 48 | never (30% smelted) | - | 522, 506, then no coal | 366 | 200 | yes |
| obvious-16 | 96 | never (30% smelted) | - | same | 366 | 400 | yes |
| obvious-16d (double chests) | 96 | never (26% smelted) | - | 453, 453 (centre two) | 382 | 400 | yes |
| **bs8** | 108 | **72.2** | **100%** | 432 each | 64 | 240 | yes (no clock) |
| **bs16** (double input + fuel chests) | 221 | **36.2** | **100%** | 216 each | 64 | 505 | yes |
| **bs8-blast** | 108 | **36.2** | **100%** of 36 | 432 each | 64 | 280 | yes |
| **bs16-blast** | 221 | **18.2** | **99%** of 18 | 216 each | 64 | 585 | yes |

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

## 16 furnaces: two modules under double chests (bs16)
- One line of 16 cannot be metered this way: 16 line hoppers hold 14 items when the last fills (one
  empty in eight), a dust line reaches 15 locks (its repeater delays half the line's window), and a
  second item reaching the last hopper during the 8 gt delay keeps the window open (b16 drafts: 46-91
  min, positions starved every other window).
- bs16 = two bs8 modules back to back: the hopper under each half of the double input chest starts
  one module's line (the two centre positions carry no furnace, so the modules' dust networks never
  touch), the double fuel chest feeds one fuel line per module (with one shared fuel line the first
  module's side hoppers took every coal), and each module's output line runs into the output chest
  between them (one output line carries 150/min: 16 blast furnaces make 192).
- The judge places a double chest across `port items:` + `port items.b:` (and `fuel` + `fuel.b`).
