# Study 9, TNT dupers: world-record targets (2026-10-20)

vjeux 12:45Z: "world record kind of implementations for every single one of these... quality over
quantity". Every record is judged with explosions on and must be robust over every kick angle
(`circuit verify` = 16 real-game builds + the redsim all-kicks sweep). The targets below come from
the 26.3 mechanism (VARIATIONS.md, "The mechanism"), not from outside designs.

## What every duper needs (the lower bound's parts)

A TNT is primed by power + an update, inside one piston push that also moves a copy of it:
1. the TNT block; 2. a pusher (a sticky piston: the cycle needs a pull too); 3. a frame the push
moves (slime or honey: the TNT and the update source must ride it); 4. the silent power: a dust
(the only power source that can reach a TNT inside a push, VARIATIONS "MOVABLE") on an immovable
block (obsidian: the dust must not ride); 5. the power whose connection changes mid-push: a
redstone block riding the frame; 6. the update: a dead coral fan riding the frame that pops when
its support's cell turns moving_piston. That is 7 distinct parts; the frame must join the TNT,
the fan and the redstone block to the piston face. The 40M-iteration local search from 12 blocks
(`circuit tnt improve`, explosions off) found nothing smaller than 12 and nothing faster than
200 TNT/min: 12 is the bound for the bare core in the searched space.

## Records and targets

| variation | current | target (why) | status |
|---|---|---|---|
| smallest survivable, self-running | 15 (tnt-launch-15b) | 12-14: the core is 12; survival needs every TNT to leave the frame's reach for every kick. A core whose TNT spawn has no frame block within 1 cell at y <= 0 drops clear (the kick drifts a TNT about 0.18 block before it falls below the frame top, but it starts level with it), so 12-13 may survive without a launcher | search (redsim, explosions, all kicks) |
| fastest | 200 TNT/min (period 6) | 300 (period 4): a sticky piston's push takes 2 gt and its pull 2 gt; 6 gt is the 12b's self-clock (extend 2 + the dust/obsidian power path + retract 2 + reset). A faster external clock (observers, 4 gt) on the same core could reach 4 gt if the priming still happens | search |
| tileable | 4 copies, pitch 4 (explosions off) | pitch 3 (the core is 3 wide) with explosions on: a launched TNT flies over the neighbours, so copies can touch side by side | build + judge |
| PRECISE + survivable | 43 (tnt-precise-slow-43b) | about 30: the aligner (wall stone, water source, channel floor and walls) on the core plus the smallest slow driver that gives one push per 81+ gt. The 7-block crafter loop releases 10 gt (2 pushes); a clock whose release is 6 gt by itself saves the 6-block hold extension | shrink (precise-shrink + the all-kicks rule) |
| TRIGGERED + survivable | 19 (tnt-triggered-launch-19b) | 15: the lever engine + launcher is 15, and an active-low input (the input's off state a redstone block holding the piston, on = air releasing it) needs no torch inverter | build |
| lever + survivable | 15 (tnt-lever-launch-15b) | 13-15: same as the smallest | search with the smallest |
| slime-free + survivable | none | a honey frame cannot fling; the TNT must drop clear (no honey within 1 cell below its spawn) or explode in water | search |

Below-bar designs (bigger with nothing better on any variation) leave the archive when a record
replaces them.
