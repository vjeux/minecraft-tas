# Track shulker-speed (study 8, clk-item, branch explore/tff): unloaders + speed for both sides

Split agreed with clk-rt (owner, explore/shulker): clk-rt keeps the judge (crates/circuit/src/shulker.rs), redsim and the loader badges; this track does the unloaders and speed/tileable builds. Judge requests go to clk-rt.

## Archived (designs/circuits/shulker/archive/, robust 16/16)

| design | items/s (BASE 64) | blocks | badges | how |
|---|---|---|---|---|
| shulker-speed-unloader-2cart | 15.00 (6x the reference) | 52 | always-on, 16-stack | 2 off-rail hopper minecarts in the cell under the box, each centred in the box's column (sucks it at 20/s) and straddling 4 of 6 hoppers |
| shulker-speed-unloader-rail | 7.50 (3x) | 57 | always-on, 16-stack, unstackable, colours, box-starved, drop-free, tileable(6) | 2 hopper minecarts on one activator rail over 3 hoppers; the empty-box pulse powers the rail in the same redstone update as the breaking piston |
| shulker-small-unloader | 2.50 | 41 (clk-rt's always-on: 49) | all of the above, tileable(5) | clk-rt's always-on unloader without the hopper lock (the hopper's 8 gt cooldown covers the break; robust over 16 seeded drop draws) and without the drop cell's side walls and the wall south of the box; a piston next to the box fails (its QC cell gets the observer's stone) |

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

## WIP: loader-dropper.txt (hopper N + gated dropper Y under the box)

First 1-stack run: box 1 full at t=141 (the hopper-only loader needs ~240), but the dropper fired after the box broke and threw a shovel into the output (fail). Cause: the gate G (subtract comparator looping through repeater R, rear = squared L > 0) only stops when L drops, i.e. 4-8 gt after the piston breaks the box, while dropper ticks are already scheduled. Fix plan: stop the gate at FULL, before the break: put a subtract comparator Z between A and the squaring (rear = A, side = K2's full signal) so the gate sees L only while 0 < L < 15, and delay the piston path a few gt (T delay 4) so the last scheduled dropper tick hits the full box (a dropper facing a full container keeps its item). B (K2's redstone block) must then move off Z's side and away from the piston dust.

Second run (Z + 18 gt piston delay, committed): boxes 1-2 full at t=157 and 321 with nothing in the output, then stuck: the judge shares the item budget across the sources (items.a first), so N's chest (items.a) ran dry after two boxes and could not seed box 3 (the gate needs L > 0), while the dropper chains held the rest; one item was still thrown (found resting on R), so a dropper tick still reaches an empty cell sometimes. Next: seed each new box without N (fire the dropper once a few gt after the dispenser succeeds, e.g. from comparator E falling), or feed N from the same chests; and widen the stop-before-break margin (or gate the dropper with box conduction: a closed shulker box is a full block and conducts redstone).
