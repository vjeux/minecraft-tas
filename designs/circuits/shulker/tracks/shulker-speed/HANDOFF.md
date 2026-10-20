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
- Loader past 7.5/s: the ceiling is 10/s, two droppers on one box, and it needs a hopper minecart under one item chest (3 chests through hoppers give only 7.5/s). DECIDED by experiment: a second dropper cannot be fired through the box. The placement one-shot works only because the dropper BELOW the box is quasi-connected to it; a dropper beside the box is not, and a weakly powered block never powers a mechanism. So each dropper needs its own two observers in opposite clock phases. The exact feasible layout is in NOTES.md ("Feasible 10/s layout"): dropper Y2 at (0,1,1) facing north with observers at (-1,1,1) and (1,1,1), fed 5/s from above and behind; Y1 fed 5/s from its west line off a cart under chest c. The build attempt stalled on routing the two clock phases up from y=-2: the dust columns need solid pillars one step out in z, since the clock dust fills the cells below.
- Mixed + restart together: a threshold on the feed hopper cannot separate them. Shovel noise reaches level 12 during a swap, while a mixed jam can plateau at 9-11, so the detector needs a persistence filter (on for more than ~64 gt): 8 delay-4 repeaters, or a hopper integrator. With the placement one-shot the first item no longer has to come from a hopper, so a dropper feeder is now possible too.
- Smallest loader: clk-rt's 31b auto-loader looks minimal for its layout. A comparator reading the box directly reads the feed hopper through the pushed stone (loader-small-directread-FAILS.txt).
