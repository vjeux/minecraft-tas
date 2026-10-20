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
on the ground. The one that is (clk-misc 2026-10-20, below) holds the engine with a catalog clock:
a 14-tick period throws each TNT before the previous ones land, and they land 9 to 30 blocks away
(7.4 to 27 on unbreakable ground), always past the 6.9-block reach; at a 10-tick period some land
5.3 blocks away and about 1 game build in 30 fails.

## The TNT duper (vjeux 21:47Z: ONE world-best implementation per kind)

**tnt-clocked-22b** (archive): 85.7 TNT/min, 22 blocks, safe on the ground, every blast clear of
water and breaking ground blocks, robust over every kick on both grounds on redsim and in 16 game
builds (5 game runs of 16 builds, all robust). Two catalog parts: the 16-block engine
(tnt-triggered-launch-16b's, without its input block) and the 6-block clock
clk-obs-p14-lamp-obs-rb-stk-6b-b-0e0eb7, whose redstone block holds the engine's piston and steps
away for 4 ticks of every 14 (the engine dupes only on such a release: a pulse on its piston primes
nothing). Found by `mctas circuit tnt hold-scan` over every archived clock. Not precise: an aligner
holds one TNT at a time (its slime launches everything on it) and an apex burst needs the launch
at fuse 16-26, so a precise duper tops out near 20 TNT/min (the air burst, 58 blocks, 13.2/min, is
a step below).

The other kind, **tnt-duper-triggered**: tnt-triggered-launch-16b (16 blocks, safe on the ground).

## Steps toward it (taken out of the archive 2026-10-20; their files stay where they were built)

| step | what it was | why it is not the one |
|---|---|---|
| tnt-launch-15b (designs/circuits/tracks/tnt/tnt-launch-15b.txt) | the bare engine running free: 200/min, 15 blocks | safe drop 5-8: on the ground its pile is within blast reach |
| tnt-slow-launch-30b | the engine on fly's crafter-loop clock + a 4-tick repeater hold: 13.2/min, 30 blocks | a bespoke hold; the catalog clock does it in 6 blocks at 6.5 times the rate |
| tnt-airburst (designs/circuits/tracks/clk-misc/tnt-airburst-58b.txt) | precise mid-air burst, 58 blocks, 13.2/min | precise caps the rate near 20/min (above) |
| tnt-switch-launch-16b | the free engine with a lever: 200/min, safe drop 4 | same pile problem as the free engine |
| tnt-drop-16b | drops each TNT straight down: safe drop 25-31 | needs a pit |
| tileable | 4 copies, pitch 4 (explosions off) | judged on the ground: open |
| slime-free | none | a honey frame cannot fling; the TNT must drop clear: open |
