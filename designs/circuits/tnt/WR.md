# Study 9, TNT dupers: world-record targets (2026-10-20)

vjeux 12:45Z: "world record kind of implementations for every single one of these... quality over
quantity". Every record is judged with explosions on, standing on the ground (grass that craters
and obsidian that does not, both), with no TNT removed anywhere and at every kick angle; its SAFE
DROP HEIGHT (vjeux 13:31Z) is the fewest air blocks under it at which it never damages itself, and
a lower safe height beats a smaller machine (docs/circuits.md, VARIATIONS.md "On the ground").

## What every duper needs (the lower bound's parts)

A TNT is primed by power + an update, inside one piston push that also moves a copy of it:
1. the TNT block; 2. a pusher (a sticky piston: the cycle needs a pull too); 3. a frame the push
moves (slime or honey: the TNT and the update source must ride it); 4. the silent power: a dust
(the only power source that can reach a TNT inside a push, VARIATIONS "MOVABLE") on an immovable
block (obsidian: the dust must not ride); 5. the power whose connection changes mid-push: a
redstone block riding the frame; 6. the update: a dead coral fan riding the frame that pops when
its support's cell turns moving_piston. The 40M-iteration local search from 12 blocks found
nothing smaller than 12 and nothing faster than 200 TNT/min: 12 is the bound for the bare core.

## Why the ground is hard (measured, `mctas circuit tnt blasts`)

A TNT blast breaks nothing beyond about 7 blocks (a ray starts at <= 5.2 and loses 0.225 per
0.3-block step in air) and pushes entities up to 8 blocks; a TNT exploding inside water (or a
bubble column) breaks nothing at all. A machine making a TNT every 6 ticks has 13 burning at once:
flung 1 block per tick they pile up 10 to 20 blocks away on the ground, and every blast throws
some of the pile back within reach. One TNT at a time (a period longer than the 80-tick fuse) or
landing far away (a drop under the machine) is safe. A 60-minute explosion-screened random search
(the 15b launcher with a foot, plus 2 to 6 blocks of slime, obsidian, observers, pistons and
redstone blocks, every duper screened standing on both grounds at 6 kicks) found no fast duper safe
on the ground.

## Records and targets

| variation | record now | target (why) | status |
|---|---|---|---|
| self-running, safe on the ground | 30 blocks, 13.2/min (tnt-slow-launch-30b: the launcher on a 91-tick clock, one TNT at a time, each lands 14 to 18 blocks away) | smaller: the clock is 7 blocks + a 6-block hold extension; a clock whose release is one push by itself saves the extension | record (archiving) |
| FAST (200/min) self-running | tnt-launch-15b, 15 blocks, safe drop 8 | **safe drop 0** at 200/min: the pile must form where its blasts cannot throw a TNT back within 7 blocks. Tried: (a) a water channel exit (the precise aligner) at full rate: knocked TNT leave the channel; (b) a roofed channel: they leave by its open end; (c) a magma bubble-column shaft 4 and 8 deep under the stop wall: the first 8-14 TNT explode harmlessly in it, then blasts at the bottom throw the next ones up and out of the top (explodes at y 6, 3.5 blocks from the machine). Next: a bent shaft (no line of sight from the bottom to the entry), or a steeper launch (a vertical slime push) landing 30+ blocks away | open |
| TRIGGERED, safe on the ground | 16 blocks (tnt-triggered-launch-16b: active-low input, obsidian foot) | 15: without the foot the slime frame drags the grass; a design whose moving slimes never rest on the ground could save it | record |
| lever (switchable) | tnt-lever-launch-15b, safe drop 6 | safe drop 0: same as the fast one | open |
| PRECISE | 43 blocks, safe on the ground (tnt-precise-slow-43b) | about 30: the aligner on the slow launcher | open |
| dropped | tnt-drop-16b, safe drop 31 (works over a deep pit only) | lower drop | open |
| tileable | 4 copies, pitch 4 (explosions off) | judged on the ground | open |
| slime-free | none | a honey frame cannot fling; the TNT must drop clear | open |
