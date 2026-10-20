# TNT dupers (study 9): the variations builds need, and how the judge measures each

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
