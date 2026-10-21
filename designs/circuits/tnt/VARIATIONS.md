# TNT dupers (study 9): the variations builds need, and how the judge measures each

**The one TNT duper (vjeux 2026-10-20 21:47Z):** tnt-clocked-22b, 85.7 TNT/min, 22 blocks, safe on
the ground, from two catalog parts (the 16-block engine held by a 6-block catalog clock); the other
archived designs became steps toward it (WR.md "The TNT duper" and "Steps toward it"). The sections
below are the study's history and measurements.

Derived from the 26.3 source (`mctas src show <Class>`) and what TNT builds use; no outside lookup.

## The mechanism (26.3 source)

* `PistonBaseBlock.moveBlocks` copies every pushed state first (`toPushShapes`, line 274), writes
  each destination as `moving_piston` with flag 324 (no neighbor update, but the destination's
  neighbors re-check their shape: `Level.setBlock` 251-255), and blanks the sources only at the end
  (line 324, flag 82). Inside that window a pushed TNT still stands at its source while its moving
  copy (holding the copied TNT state) already exists.
* An update that reaches the TNT inside the window primes it if it is powered at that moment
  (`TntBlock.neighborChanged` 53-59: `PrimedTnt` at the source cell, then `removeBlock`), and the
  moving copy still lands as a TNT block 2 gt later: 1 TNT block in, 1 TNT block + 1 primed TNT
  out.
* Updates inside the window: a block whose support just became `moving_piston` fails `updateShape`
  and is destroyed with full updates (`Block.updateOrDestroy` 221-224 -> `destroyBlock`, flags 3).
  A support-needing block that rides the structure pops at its source and its moving copy lands
  again: a reusable update. Dead coral fans have no `POPPED` reaction (`Blocks` 4160: pushable,
  slime drags them) and drop nothing; rails do the same but drop a rail item every time. Also a
  powered observer whose source is overwritten (`LevelChunk.setBlockState` 317-318, flag 64:
  `ObserverBlock.affectNeighborsAfterRemoval` -> `updateNeighborsInFront`).
* Power: a TNT never rests next to power (it primes on `onPlace` and on the landing's own
  `neighborChanged`, `PistonMovingBlockEntity` 313-314), so the signal must appear in the window
  without updating it first. Only dust computes its signal from its current neighbors
  (`RedstoneWireBlock.getSignal` 361-375); a strong source landing updates its own neighbors, not
  its target's. Which of these the smallest dupers use is what the first searches answer
  (redsim census, then the real game).
* The primed TNT (`PrimedTnt` 66-76): position (x+0.5, y, z+0.5) of the source cell; motion
  (-sin r * 0.02, 0.2, -cos r * 0.02) with r uniform in [0, 2 pi) (a random horizontal kick of
  exactly 0.02); fuse 80. Each tick: gravity 0.04, move with collision (a hit horizontal component
  becomes 0: TNT has no bounciness, `Entity.restituteMovementAfterCollisions`; a slime floor bounces
  it), drag 0.98, on ground x0.7 / y x-0.5 / z x0.7; explodes after the 80th move at
  (x, y + 0.06125, z). A piston pushes it to the moving face + 0.01 on the push axis; a moving slime
  sets that axis' motion to +-1; a honey block moving sideways carries it if it stands on top. TNT
  primed by an explosion instead gets a random fuse of 10-29 (`getRandomShortFuse`).

## Judge kinds

* `tnt-duper`: self-running (an optional `port start:` kick, like the clocks). Pass: the whole
  design is periodic (period P, as a clock) and spawns primed TNT every period, while the design's
  TNT blocks are all present at every period boundary, for >= 8 periods at 16 builds (4 positions
  x 4 rotations). Metrics: TNT per period, period, TNT per minute (1200 x count / P), latency
  (kick to first spawn), blocks (the TNT block counts), box.
* `tnt-duper-triggered`: `port input:` (button, block or lever). Each input pulse spawns the same
  k >= 1 primed TNT, the design returns to rest with its TNT intact. Metrics: k, latency (edge to
  spawn), minimum pulse period (the fastest it can be re-fired), blocks, box.
* With `travel:` either kind is a movable duper: the flying-machine checker (track fly's tracker in
  `crates/circuit/src/fly.rs`) plus the spawns; positions are taken relative to the machine.
* Every primed TNT is tracked from its spawn tick to fuse 1 and removed before the tick it would
  explode in (explosions never happen; the record keeps tick, position and motion at spawn and at
  fuse 1). `measure: spawn` removes it at spawn instead.

## Variations

| variation | builds that need it | what the code says | judge metric (pass rule) |
|---|---|---|---|
| smallest | any | the copy window needs a TNT, a pusher, an update source and a power source in the same push | blocks, then box volume |
| rate | world eaters, bores, farms | one push dupes every TNT block it updates while powered; the period of the pushing engine sets the rest | TNT per minute; period P |
| PRECISE | cannons, TNT stacking, alignment | spawn position is exact (cell center), the motion is not: a random 0.02 kick. Cancelled by: a push across both horizontal axes (piston faces + 0.01), a slime launch on the kicked axis (motion becomes +-1, position still kicked), walls 0.01 away (zero the hit axis, leave +-0.01) | spread (max - min over >= 64 spawns) of position and motion at fuse 1, per axis, relative to the duper; 0 = perfect; also at spawn |
| direction / motion | world eaters (straight down), cannons (launched), placement (stationary) | free fall from 0.2 up; a moving slime launches at 1 block/gt on its axis | mean motion over the first 4 gt and at fuse 1; class: drop, launched (+axis), stationary (at rest on a floor) |
| BURST / stacked | cannons, compression | several TNT blocks in one push are duped in the same tick at different cells; the same exact position needs alignment after spawn | max TNT spawned per tick; coincident: TNT sharing one exact position at fuse 1 |
| fuse phase | cannons, chained timing | power-primed TNT always gets fuse 80; explosion-primed TNT gets 10-29 at random (to avoid) | spawn ticks mod P (one phase or several), all fuses = 80 |
| control: continuous | world eaters, farms | self-running clock or engine | `tnt-duper` |
| control: per pulse | cannons, single shots | one push per input pulse | `tnt-duper-triggered`: k per pulse, latency, minimum re-fire period |
| control: lever on/off | farms, switchable machines | runs while the input is on | `tnt-duper` with `port input:` held: rate while on, no spawn later than L gt after off, restarts on the next on |
| MOVABLE | world eaters, trenchers, tunnel bores | the duper rides a flying machine and dupes every cycle | flying-machine pass + TNT per block moved; precision relative to the machine; direction class |
| flat / 1-wide | trenchers (many in a row), tight builds | | box height 1 or width 1 |
| TILEABLE | trenchers, quarries (a row of dupers) | copies side by side must not power or push each other | n copies (default 4) placed at the box's width along each horizontal axis: every copy passes with its own rate |
| survivable | world eaters, real use with explosions on | the TNT must explode away from the duper, under water, or behind blast-resistant blocks | a second run with explosions on (no removal): after 8 periods the design's blocks are unchanged |
| slime-free / honey-only | honey machines, mixed-glue builds | honey and slime never stick to each other (`PistonStructureResolver.canStickToEachOther` 68-73) | composition of the design |
| item-free | lag | a popped rail drops an item every dupe; coral fans drop nothing | item entities after 8 periods = 0 |

The best design of each variation is archived (`mctas circuit archive`); a design that is bigger
and slower than an archived one of the same variation is dominated and stays out.

## On the ground, and the SAFE DROP HEIGHT (vjeux 13:13Z, 13:31Z): the current judge

vjeux on tnt-launch-15b: "There's no way this tnt duper doesn't explode after a few seconds". He
was right: the first explosion judge ran in a void sandbox, where every TNT that left the machine
fell forever and blew up 70+ blocks below. On a floor one block under the machine (`mctas circuit
tnt blasts`), the dropping and launching dupers broke within seconds: a dropped TNT lands right
beside the machine; launched ones pile up 10-20 blocks away and every blast throws some of the
pile back within the ~7 blocks a TNT blast can break. So every TNT duper is now judged standing
on a ground (grass over dirt over bedrock, which craters like a real world, and obsidian, which
does not; both must pass), with no TNT removed anywhere (each explodes where it lands; the ground
grows until no TNT leaves it), at every kick angle, and the judge measures its SAFE DROP HEIGHT:
the fewest air blocks between the machine and the ground at which it never damages itself (0 =
safe on the ground; more than 40 = fails). A lower safe height beats a smaller machine.

| design | blocks | rate | safe drop | where its TNT explode (16 kick angles, both grounds) |
|---|---|---|---|---|
| tnt-triggered-launch-16b | 16 | one per pulse | **0: safe on the ground** | 14.2 to 18.5 blocks away (grass), 14.2 (obsidian); 0 machine blocks destroyed |
| tnt-slow-launch-30b | 30 | 13.2/min | **0: safe on the ground** | 14.2 to 18.5 (grass), 14.2 (obsidian): one TNT at a time (a 91-tick clock) |
| tnt-precise-slow-43b | 43 | 13.2/min, precise | REMOVED (2026-10-20, the usefulness rule) | all in its own water channel (water: no block breaks): every TNT explodes in water, so it is useless (designs/circuits/tnt/tracks/rejudged/) |
| tnt-lever-launch-15b | 15 | 200/min, switchable | 6 blocks | (at its safe drop) 13.7 to 49.3 (grass), 9.5 to 41.3 (obsidian) |
| tnt-launch-15b | 15 | 200/min | 8 blocks | (at its safe drop) 11.8 to 54.3 (grass), 11.7 to 59.1 (obsidian); 0 destroyed over 10 minutes |
| tnt-drop-16b | 16 | 200/min, dropped | 31 blocks | (at its safe drop) 23.6 to 35.7 blocks away: it only works over a deep pit |
| tnt-launch-14b | 14 | 200/min | 40 blocks | below bar: the 15b needs a lower drop |

Open record: a fast (200/min) duper that is safe ON the ground (its TNT must land far enough that
the pile's blasts can never throw one back within ~7 blocks: a steeper launch or a water landing).

## Explosions on (vjeux 2026-10-20 11:53Z): the re-judge

"You didn't model tnt explosion. This will blow up the machine." Every TNT duper is now judged
with its explosions (the default; `explode: off` is analysis only and is not archived). The eight
designs archived before, in the real game at 16 builds:

| design | with explosions | now |
|---|---|---|
| tnt-duper-12b (smallest) | BLOWN UP: some TNT land on its frame (the 0.02 kick drifts a TNT up to 1 block) | out of the archive (tracks/tnt, `explode: off`) |
| tnt-duper-triggered-13b | BLOWN UP (a destroyed block's item at the end) | out; survivable triggered: open |
| tnt-lever-12b | BLOWN UP | replaced by archive/tnt-lever-launch-15b (survives: every TNT out of range) |
| tnt-honey-15b (slime-free) | BLOWN UP | out; survivable slime-free: open |
| tnt-precise-28b | survives (water channel), but NOT precise any more: the explosions knock the next TNT (spread 6.3 blocks) | out (same blocks as survive-28b) |
| tnt-survive-28b | survives (44 exploded in water, 45 out of range) | out: dominated by the 15-block launcher |
| tnt-launch-15b | survives: all 96 TNT leave out of range | archived: the SMALLEST survivable duper |
| tnt-drop-16b | survives: all TNT fall straight down out of range | archived |

PRECISE under explosions: DONE by slowing it down (coordinator), then REMOVED (2026-10-20: every TNT
of it explodes in its water aligner, and the judge's usefulness rule fails a duper whose TNT explodes
in water; the precise record is tnt-airburst, which launches each TNT out of the water first: 58
blocks since 2026-10-20, designs/circuits/tracks/clk-misc/tnt-airburst-58b.txt, after fly's 75-block
tnt-airburst-75b. Straight up from an aligner right at the duper (2 blocks north) the TNT peaks
only ~6 blocks above the machine's top slime, inside the blast's 6.9-block reach, so the aligner sits
4 blocks north; its x stop is a fence post, not a wall, so the TNT rests over obsidian (no slime
slowdown) and the first launch tick, still sliding along the post, closes the last 0.0003 of a block
that a TNT pushed by the current can stop short of (Entity.move drops a collided move under 3.2e-4
blocks): every kick ends at one point):
tnt-precise-slow-43b — the
precise water-channel aligner on a slow engine (a 7-block crafter-loop clock, period 91 gt, holds
the sticky piston and releases it for 6 gt: one push a period), so one TNT is alive at a time
(91 gt > its 80 gt fuse): no blast knocks the next one, each goes off in the water (no block
damage). Game, 16 builds: 20 exploded, design intact, "precise: 19 TNT at one point at fuse 1",
at rest 33+ gt. 13.2 TNT/min. The judge's self-running run is now 1800 gt (8 periods of up to
110 gt in its second half).

TRIGGERED survivable: DONE: archive/tnt-triggered-launch-19b — the lever engine plus the launcher,
held by a redstone torch that each input pulse turns off (dust + stone) for one push; every TNT
flung out of range. The 13b's own TNT was pushed back onto its frame and chain-primed it.

SLIME-FREE survivable: open (honey cannot fling; a dropped TNT spawns level with the frame top and
the kick drifts it over a neighbour frame block within 2 ticks).

Next (coordinator 12:02Z, vjeux "pass every possible random number"): DONE for the kick:
`circuit verify` judges every TNT duper on redsim at every kick angle (4096-point sweep, every
change bisected to 1e-12, rays at their largest roll); all five archived designs are robust over
every kick (one piece each: the outcome never changes with the kick), and the old triggered 13b
fails for kicks in [0.1667, 0.6250) (blown up), which is what the game showed. Per-spawn kicks
(different TNT, different kicks) are the next step.

## Results so far (2026-10-20, explore/tnt; explosions OFF, kept for the record)

| variation | status | design / measure |
|---|---|---|
| smallest, self-running | 12 blocks, 200 TNT/min, P6, robust | archive/tnt-duper-12b (local search `circuit tnt improve`: 3.5M distinct dupers seen, none under 12 in this family) |
| smallest, per pulse | 13 blocks, 1 TNT per pulse, latency 1 gt, robust | archive/tnt-duper-triggered-13b |
| rate | 200 TNT/min per duper: the sticky piston's push + pull take 6 gt, 1 TNT each | tiles below scale it |
| TILEABLE | yes, robust in the game: 4 copies touching along the push axis (step 4, 3x2x16) or with a 1-block gap across it (step 4, 15x2x4): 800 TNT/min; touching across (step 3) or stacked: they interfere | `circuit tnt tile` + `circuit verify` |
| direction | DONE, all three, judge flags: LAUNCHED archive/tnt-launch-15b (a slime flings every TNT north at exactly vz = -1.0, vy = 0.1568 blocks/gt; only the cross axis keeps the random kick, spread 0.039); DROPPED straight down archive/tnt-drop-16b (the same + one wall stone: z and its motion exact, then it falls straight down, horizontal motion under 0.01 at fuse 1, 72+ blocks); STATIONARY archive/tnt-precise-28b counts: every TNT at rest for 33+ gt before fuse 1 at one point. The 12b drops most of its TNT but not all (some come to rest on its frame) | judge flags `launched at (...)`, `dropped: ...`, `stationary: ...`; `circuit tnt precision` |
| PRECISE | DONE: archive/tnt-precise-28b (28 blocks, 200 TNT/min, robust): every primed TNT at exactly (3.51, -1.0, -1.51) relative to the design at fuse 1 (85 of 85 in the judge's run, 96 of 96 with `circuit tnt precision`, all 40 fixed kicks on redsim). A slime carried over the TNT cell during the push flings every fresh TNT north at exactly vz = -1 (whatever its kick), a wall stops it (z exact, motion zeroed), a 1-wide water channel flowing east holds it against the east wall (x exact) on the floor (y exact) | judge flag `(precise: N TNT at one point at fuse 1)`; `circuit tnt precision`, `precise-sim` |
| survivable | DONE: archive/tnt-survive-28b: the precise duper judged with `explode: on`: 64 explosions in the judge run, the design intact and still duping in all 16 builds. Every TNT explodes inside the water channel; water (blast resistance 100) stops every ray at its first step | judge header `explode: on`: the game run lets the TNT explode; flag `(survives its own explosions: N exploded, design intact)` |
| item-free | yes: every archived duper (dead coral fans drop nothing; the judge fails a run with any item entity at the end) | |
| flat / 1-wide | FLAT: archive/tnt-duper-12b is 2 high (3x2x4), the least possible: inside a push only a dust can give a TNT silent power, and dust needs a block under it. 1-WIDE is impossible: the dust must be powered at rest with its TNT perpendicular to its power source (a dust with one connection points both ways along it, so a TNT in line would be powered at rest and prime) and only changes shape when that source's cell turns moving_piston, so the dust, its source and the TNT form an L across both horizontal axes. 2-wide: not found (700M redsim samples in 1- and 2-wide boxes), not proven impossible | box (dims in the judge line) |
| MOVABLE | not found, and the code leaves no room for it: inside a push, only a dust's connection change (computed on the fly) gives a TNT power without updating it first, and dust cannot ride a piston; the other silent source is an observer landing powered on a cell with a pending observer tick, but a landing happens after that tick's phase, so the tick an observer scheduled there is already gone | analysis (track NOTES.md) |
| BURST | same tick, different cells: the tiled 12b primes 4 TNT in the same tick from 4 engines (robust). One push priming 2 TNT: a second dust beside the same redstone block, its TNT and its fan push the frame past the 12-block push limit; 12.8M redsim samples from the 12b found none. Same position needs the aligner of PRECISE | `circuit tnt search --min-per 2` |
| lever on/off | DONE: archive/tnt-lever-12b (the 12b + an input beside its sticky piston: the input held on keeps the piston extended, nothing dupes; off, it runs at 200 TNT/min, first TNT 5 gt later; robust) | `tnt-duper` with `port input:`: off / on / off / on windows of 300 gt; dupes in exactly one input state, stops within 40 gt in the other; metrics `runs input on|off`, `stop`, `start` |
| slime-free / honey-only | DONE: archive/tnt-honey-15b (15 blocks, 200 TNT/min, robust; judge flag `slime-free: a honey frame`): the 12b engine with a honey frame. Honey gives a fan no sturdy face, so a dead coral WALL fan hangs on a stone that rides the frame; the honey under the fan holds it, and it pops when the stone it hangs on moves away during the push, while the TNT beside it is still in its old cell (an earlier note here called honey-only impossible: wrong, the fan hangs on a stone, not on honey). Frame-free (only pistons in a line, no glue): not found; a fan riding a plain line moves into its support's cell before it can pop | judge flag `slime-free` (no slime block in the design) |
