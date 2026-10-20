# HANDOFF: track shulker-speed (study 8, clk-item, branch explore/tff)

Split with clk-rt (owner of study 8, branch explore/shulker): clk-rt keeps the judge (crates/circuit/src/shulker.rs), redsim, the loader badges. This track: unloaders, speed and tileable for both sides, the combo. Mechanics, failed tries and pitfalls: NOTES.md next to this file.

## Archived (designs/circuits/shulker/archive/, all robust incl. the drop stress, 2026-10-20)

| design | kind | rate | blocks | badges | track file |
|---|---|---|---|---|---|
| shulker-speed-unloader-rail | unloader | 7.50 items/s (3x) | 57 | always-on, 16-stack, unstackable, colours, box-starved, drop-free, tileable(6) | unloader-rail.txt |
| shulker-small-unloader | unloader | 2.50 | 41 | the same, tileable(5) | unloader-small.txt |
| shulker-speed-loader-dropper75 | loader | 7.39 items/s (3x; the ceiling of hoppers under 3 item chests) | 108 | always-on, 16-stack, unstackable, colours, restart, top-up, box-starved, drop-free, tileable(9) | loader-dropper75.txt |
| shulker-combo | loader + unloader (`port input.mode` lever) | 2.50 | 85 | the loader badges above + combo | combo.txt |

Removed: shulker-speed-unloader-2cart (15/s but fragile: off-rail carts swallow unstackable drops), shulker-speed-loader-dropper (dominated; its 5.74 was the old source-outflow metric), clk-rt's 49b auto-unloader (dominated by the 41b one, at clk-rt's request).

## What is open

- Faster unloader than 7.5/s that survives the drop stress: carts on activator rails cap at 3 hoppers per box (both carts must sit in the box's column on one rail cell). More needs parallel boxes (two stations sharing the chests).
- Loader past 7.4/s: hoppers under the 3 item chests cap the supply at 7.5/s; a hopper minecart under a chest pulls 20/s.
- Flush on the fast loader: a flush that fires when the feed runs dry also fires in the restart badge's 3000 gt pause; it needs a > 3000 gt timer to have both. Mixed: a full-box comparator never reads 15 with partial stacks of several items.
- Smallest loader: clk-rt's 31b auto-loader looks minimal for its layout; a comparator reading the box directly reads the feed hopper through the pushed stone (loader-small-directread-FAILS.txt).
- Combo is 85b: the mode lever's two lock lines (full-input lock at the detector, torch under the unloading hopper) are far apart; a layout with the detector near the unloading hopper would save ~15 blocks.
