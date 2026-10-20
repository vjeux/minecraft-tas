# HANDOFF: track shulker-speed (study 8, clk-item, branch explore/tff)

Split with clk-rt (owner of study 8, branch explore/shulker): clk-rt keeps the judge (crates/circuit/src/shulker.rs), redsim and the loader badges. This track covers unloaders, speed and tileable for both sides, the combo, mixed, and flush with restart. Mechanics, failed tries and pitfalls are in NOTES.md next to this file.

## Archived (designs/circuits/shulker/archive/, all robust incl. the drop stress, 2026-10-20)

| design | kind | rate | blocks | badges | track file |
|---|---|---|---|---|---|
| shulker-speed-unloader-rail2 | unloader | 11.62 items/s | 116 (shrunk in the game from 122) | always-on, 16-stack, unstackable, colours, box-starved, drop-free, tileable(12) | unloader-rail2.txt |
| shulker-speed-unloader-rail | unloader | 7.50 items/s (3x) | 57 | the same + tileable(6) | unloader-rail.txt |
| shulker-small-unloader | unloader | 2.50 | 41 | the same, tileable(5) | unloader-small.txt |
| shulker-speed-loader-dropper75 | loader | 7.39 items/s (3x; the ceiling of hoppers under 3 item chests) | 96 (shrunk in the game from 108) | always-on, 16-stack, unstackable, colours, restart, top-up, box-starved, drop-free, tileable(9) | loader-dropper75.txt |
| shulker-speed-flush-loader | loader | 7.40 items/s | 127 | always-on, 16-stack, unstackable, colours, RESTART + FLUSH, top-up, box-starved, drop-free (rotation-dependent by 3-5 gt) | loader-flush.txt |
| shulker-mixed-loader | loader | 2.50 | 57 | always-on, 16-stack, unstackable, colours, top-up, flush, MIXED, box-starved, drop-free | loader-mixed.txt |
| shulker-restart-flush-loader | loader | 2.50 | 65 (shrunk in the game from 75) | always-on, 16-stack, unstackable, colours, RESTART + FLUSH, top-up, box-starved, drop-free | loader-rs.txt |
| shulker-combo | loader + unloader (`port input.mode` lever) | 2.50 | 79 (shrunk in the game from 85) | the loader badges + combo | combo.txt |

Removed: shulker-speed-unloader-2cart (15/s but fragile: off-rail carts swallow unstackable drops), shulker-speed-loader-dropper (dominated; its 5.74 was the old source-outflow metric), clk-rt's 49b auto-unloader (dominated by the 41b one, at clk-rt's request).

## Key mechanisms

- rail2: two rail stations share one boxes chest (the judge puts every box in the first boxes chest). The splitter is a hopper minecart under that chest: two hoppers under the cart pull at the same time, so boxes 1 and 2 go to different stations. A plain hopper puller under the chest's hopper grabs every box, because it pulls before the other hopper's push.
- mixed: the jam detector reads the feeding hopper's comparator through a 4-dust chain (on at level >= 4). A subtract comparator turns its rising edge into a 4 gt piston pulse. Jam => 27 occupied slots, since a box refuses an item only with no empty slot.
- restart + flush: an hourglass (a chest of 420 stone over a hopper, draining at 2.5/s). It runs only while the items chest is empty AND the dispenser holds boxes: 3360 gt, longer than the 3000 gt pause and within the 6000 gt deadline. Its expiry lights a dust that fires the piston.

## What is open

- Mixed AND restart together: the plain-hopper backlog jam detector is noisy with shovels. N holds 2-4 shovels around swaps and 0 in the pause, and one shovel is worth 3 comparator levels, so a crossing after a placement breaks the new empty box (loader-mixed-rs.txt, WIP). It needs a feeder that drains faster than it is fed: a dropper firing every 4 gt, fed by one hopper.
- Fast loader with flush + restart: the hourglass works, but on dropper75 its two lock signals (feed active, dispenser empty) and its output are far apart. The output must go into K2's output line (the fake-full also closes the gate). Injecting into the piston line lets the dropper fire into the pushed stone.
- Loader past 7.5/s: the ceiling is 10/s (two droppers on one box) and the design is specified block by block in NOTES.md ("10/s loader: where the build stopped"), with the WIP file loader-10s.txt beside it. The pieces are all proven: a failed shulker-box dispense is harmless (OptionalDispenseItemBehavior returns the stack, no entity), and an observer firing into a conductor under both droppers fires BOTH, since the box's QC neighbour check sees it. Dropper Y2 goes at (0,1,1) facing north, (0,0,1) becomes stone and carries the observer pulses, Y2 is fed 5/s from (0,2,1) and (-1,1,1), Y1 5/s from (-1,0,0) and a new hopper at (0,0,-1). Supply must be exactly TWO item chests, each over a hopper minecart at a cell edge with two hoppers under it: three chests cap at 7.5/s and take the whole 81-stack budget at t=0, so nothing is ever topped up. Where it stopped: routing the two clock phases up to the observers; build the second phase at y=0, one level up, not at y=1.
- Reading a design: take its blocks from its trace file (designs/circuits/shulker/traces/<name>.trace, line 2 lists every cell with coordinates), never by parsing the layer grid against the caption: `to_text()` reassigns palette letters on every write.
- Mixed + restart together: BLOCKED on a read path, with the cause measured (NOTES.md, "Mixed + restart: measured blocker"). A persistence filter is built and works (loader-mixed-restart.txt, 97b: threshold comparator, 8 delay-4 repeaters, torch, AND comparator), but no level threshold can separate a jam from the backlog, because a hopper feeder's backlog is permanent: N fills before the first box is placed and stays high after it. The gate that would fix it, "the box is nearly full", has no reader left: the box has one readable face (north, through the stone) and the full detector already uses it. The fix is a dropper feeder that drains faster than it is fed (empty except in a jam), which needs the placement one-shot, so it must be built on the flush loader (127b). There all five non-box faces of the dropper are occupied, so the next step is to move the collection hopper K one cell east and read the dropper through the freed stone.
- Smallest loader: clk-rt's 31b auto-loader looks minimal for its layout. A comparator reading the box directly reads the feed hopper through the pushed stone (loader-small-directread-FAILS.txt).
