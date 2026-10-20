# Track shulker-speed (study 8, clk-item, branch explore/tff): unloaders + speed for both sides

Split agreed with clk-rt (owner, explore/shulker): clk-rt keeps the judge (crates/circuit/src/shulker.rs), redsim and the loader badges; this track does the unloaders and speed/tileable builds. Judge requests go to clk-rt.

## Archived (designs/circuits/shulker/archive/, robust 16/16)

| design | items/s (BASE 64) | blocks | badges | how |
|---|---|---|---|---|
| shulker-speed-unloader-2cart | 15.00 (6x the reference) | 52 | always-on, 16-stack | 2 off-rail hopper minecarts in the cell under the box, each centred in the box's column (sucks it at 20/s) and straddling 4 of 6 hoppers |
| shulker-speed-unloader-rail | 7.50 (3x) | 57 | always-on, 16-stack, unstackable, colours, box-starved, drop-free; tileable(6) with `tile: 0 0 6` (track file) | 2 hopper minecarts on one activator rail over 3 hoppers; the empty-box pulse powers the rail in the same redstone update as the breaking piston |

## Mechanics (26.3 source, verified in the game)

- A hopper pulls from any container ENTITY whose box overlaps the 1x1x1 cube above it (a random one each try). A hopper minecart (0.98 wide) straddling a cell edge feeds 2 hoppers; off-rail at a corner, 4.
- A hopper minecart sucks the block container at containing(x, y + 1.5, z): its centre must be inside the box's column, one cell under the box. 1 item/gt.
- With no container above, a hopper minecart sucks item entities in the 1x1 column from 0.69 to 2.0 above its own level, i.e. inside the box cell: it swallows the broken box's drop in the very tick it spawns (block events run before entities) unless it is full or disabled.
- Off-rail carts cannot be disabled. They are safe only when every slot still holds items at break time: true for 64- and 16-stack items, false for unstackables (a slot empties within a few gt).
- An activator rail powered in the same redstone update as the breaking piston disables carts on it before they tick (activateMinecart runs in the cart's tick before it sucks).
- Stationary carts keep the yaw they were spawned with. Two carts push each other only if |direction . facing| >= 0.8 within 0.2 of each other, so yaw 45 lets carts sit side by side in x or z.
- Never put a cart centre exactly on a cell edge (z = 0.0): a 90 degree turn maps it to -0.0 and the wrong column (rotations failed until 0.01).
- Loader face budget with the dispenser above the box: dispenser, piston stone, drop exit, comparator read-through and ONE feeding hopper (a hopper under the box would pull). So a hopper loader is capped at 2.5 items/s per box. More needs droppers (5/s per face, dropper under the box facing up), which must be gated: a dropper facing air throws its item. Gate idea: subtract comparator, rear = the box level L, side = a clock (pulses only while L > 0), first item from the hopper. Tried a second hopper with the drop leaving downward (loader-2h.txt): the drop escapes, no good.
- Rail unloader tiles at a 6-block pitch; the off-rail one cannot get `tileable` (the badge runs with unstackables).
