# 3x3 piston door

The goal is the 3x3 piston door with the fewest blocks, built from first principles against the
real game code. The record is **40 blocks**:
[fc2](../designs/piston-door-3x3/archive/fc2-40b.txt), by the fresh-c agent on the night of
2026-10-18 ([GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.gif),
[video](https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.mp4)). Two other
40-block doors tie it: riders' [fx1](../designs/piston-door-3x3/archive/riders-fx1-40b.txt) (fc2
with its left piston toggled by three pulses per flip) and wire21's
[m05](../designs/piston-door-3x3/archive/m05-21b-sym-40b.txt) (a different, pulse-only mechanism).
The story of that night is in [overnight-2026-10-18.md](overnight-2026-10-18.md). Rankings are in
[LEADERBOARD.md](../designs/piston-door-3x3/LEADERBOARD.md), the best of each board as a GIF in
[GALLERY.md](../designs/piston-door-3x3/GALLERY.md), and every verified design in the
[archive](../designs/piston-door-3x3/archive/README.md).

## The rules (what `mctas door verify` checks)

A design names two ports: `door`, the minimum corner of the 3x3 opening, and `lever`. The opening
lies in the plane `z = door.z` and spans x and y from that corner to +2. The checks live in
`crates/piston-door/src/lib.rs`.

* **Legal blocks.** Nothing a survival player cannot place: no command or structure blocks,
  barriers, light, bedrock, spawners, portals, `moving_piston`, fire, infested blocks... (the
  `FORBIDDEN` list), and no piston head without its extended piston. Every other block and
  mechanic is allowed.
* **It settles.** The design is placed as written, wire shapes are computed, and every block gets
  one update. It must then go idle.
* **Every block can stand where it is.** In the settled rest state every block must pass the
  game's own `canSurvive` check, as it would have to for a survival player: no lever, torch, dust
  or repeater on air. (The minimizer once produced a floating lever.)
* **Nothing changes by itself.** No block may take random ticks (`isRandomlyTicking`): unwaxed
  copper oxidizes, leaves decay, crops grow. Use waxed copper: fresh-d's unwaxed copper bulb
  oxidized, its observer saw the change, and the door fired by itself at cycle 20 of a 40-cycle run.
* **Closed at rest:**
  * the 9 door cells hold 9 identical full blocks, and none is a mechanism block (slime, honey,
    pistons, observers, redstone blocks, lamps, targets, repeaters, comparators, dust, torches,
    levers, dispensers, droppers, hoppers, note blocks, TNT);
  * the **walkway** has no collision. It is the opening extended 3 blocks in front of and 3 behind
    the door, 54 cells;
  * the `lever` port holds an unpowered lever.
* **Open.** The lever is right-clicked and the build runs until idle. Then the 9 door cells and the
  walkway must have no collision and hold no moving block, with no entities.
* **Close.** The lever is right-clicked again and the build runs until idle. Then the door cells
  must hold their rest blocks, the whole watched box (the design plus the walkway plus a 2-block
  margin) must be identical to the rest state, and there must be no entities (nothing broke or
  dropped).
* **3 cycles at every position.** Dust updates its neighbors in a position-dependent order, so a
  door must work wherever it stands. Plain `verify` uses 4 world offsets. A record counts only if
  `door verify <design> --extra-offsets 28` passes from main's code: 32 positions.
* **Score**: the number of non-air blocks, piston heads excluded.

## Categories and boards

`door verify` prints `category solid|gravity|sculk|entity`, plus `, slime-free` and `, flat` when they apply.
`mctas door leaderboard` verifies every design on main and on the agents' `explore/*` branches with
main's verifier and writes the boards.

* **solid**: the main ranking. Everything that is neither gravity nor sculk.
* **gravity**: the door blocks are sand, red sand, gravel, suspicious sand or gravel, or concrete
  powder. Such a door can fall, so it ranks separately.
* **sculk**: the design contains a sculk sensor. Footsteps or mobs nearby can trigger one, so it
  ranks separately. Gravity wins if both apply.
* **entity** (since 2026-10-18, vjeux): the design declares entities, as `entity:` lines in the file
  (`entity: minecraft:minecart 0.5 1.0625 3.5 [{snbt}]`: type, world position as decimals, optional
  data). Every other board bans entities; "Entities allowed" ranks these together with the solid
  designs (a gravity or sculk door with entities ranks nowhere). The judge, only for such designs:
  * each entity counts as one block;
  * allowed types: minecarts (not command block or spawner minecarts), boats and rafts, armor
    stands, item frames, paintings: what a survival player places and what never acts on its own
    (no mobs: their AI is random; no items or arrows: they despawn); allowed data keys: `Rotation`,
    `Items`, `equipment`, `Item`, `ItemRotation`, `Facing`, `variant` (no `NoGravity`, `Tags`,
    passengers, poses: only commands set those);
  * the entities are summoned after the blocks (deterministic UUIDs and entity randoms); settling
    waits until each is at rest (the last tick did not move it, no motion left: a rolling minecart
    only slows by 0.96 per tick, so it must stop on an unpowered powered rail or against a block);
  * at rest: exactly the declared entities, none in front of or behind the door (the same 3x3
    columns at any distance as blocks), none with collision (boats, pushable minecarts) in the
    walkway;
  * open: no entity with collision in the opening or the walkway (the open tick counts it), the same
    entity set (nothing dropped, nothing broke);
  * close: every entity back EXACTLY at its rest position (rails put a cart back on the rail's center
    line at the rail's height + 1/16 and pistons move entities by exact half-block steps: nothing in
    the source jitters a resting entity, so no tolerance), at rest, with the same state (its saved data
    without position and motion: rail, contents, pose...); no other entity at any tick, and no entity
    ever outside the watch box.
  Designs without entities are judged exactly as before.
* **Nothing in front of or behind the door** (since 2026-10-18 16:40Z): at rest, the 3x3 columns in front
  of and behind the opening, at any distance, hold no block at all (no machine parts, dust, torches,
  rails or levers). Designs judged before the rule keep their verdicts for now.
* **Rotations** (not a rule): every working design is also run turned 90, 180 and 270 degrees at the
  same positions (`door verify` prints `rotation-safe` or `NOT rotation-safe`; the game's own
  `BlockState.rotate` turns every block). Redstone updates neighbors in a fixed direction order, so a door
  can work facing one way only. The boards rank rotation-safe designs; the others are listed on their
  own board, "Not rotation-safe", with the first turn that breaks them.
* **Solid, no slime or honey**: a board, not a category. It lists solid designs with no slime or
  honey block anywhere.
* **Solid, flat (one z layer), no slime or honey**: a board. Slime-free solid designs whose every
  block (door, pistons, wiring, lever) lies in the door's plane `z = door.z`, so the whole machine is
  one block thick. `door verify` prints `, flat` when every block is in that plane.
* **Walkway clutter**: blocks without collision (dust, torches, levers, rails...) may stand in the
  walkway. The boards show how many do at rest; "clean" means none.
* **Speed boards**: fastest open, fastest close and fastest open + close, for any category and for
  solid only. Ties go to fewer blocks.
  * open = the first tick after the flip from which the door cells and walkway stay walkable (a
    moving block counts as blocking);
  * close = the first tick from which the 9 door cells hold their rest blocks;
  * both are the worst of the 3 cycles at the first position (the design as written). Other
    positions can differ by a few ticks: gravity g10 opens in 3 to 7 depending on where it stands.

## The current best: fc2 (40 blocks)

Open 24 ticks, close 9 (idle at 26 / 24). Below, t is game ticks after the lever flip, read from
`mctas door trace designs/piston-door-3x3/archive/fc2-40b.txt --close`.

### Layout

```
z = 0, the door plane (x grows to the right, y up)        z = 1 (south of it, above the walkway)

  y\x  -3 -2 -1  0  1  2  3                                 y\x  -3 -2 -1  0  1
   6    .  .  j  w  .  .  .                                  4    .  K  q  Q  $
   5    .  .  b  o  B  .  .                                  3    .  .  k  .  .
   4    l  L  a  X  A  F  s
   3    p  .  .  f  h  s  s
   2    P  .  s  #  T  #  s
   1    .  .  s  #  C  #  s
   0    .  .  s  #  M  #  s
  -1    .  .  .  .  .  m  .
  -2    .  .  .  .  G  g  .
```

| symbol | name | block, at (x, y, z) | job |
|---|---|---|---|
| `#` `T` `C` `M` | door | 9 stone, x 0..2, y 0..2, z 0 | T top-middle (1,2,0), C center (1,1,0), M bottom-middle (1,0,0) |
| `s` | left structure | 3 slime at (-1, 0..2, 0) | glued to the left door column |
| `s` | right structure | 6 slime at (3, 0..4, 0) and (2,3,0) | glued to the right door column. (2,3,0) drags T |
| `P` | P_L | sticky piston facing east, retracted, (-3,2,0) | pull-type pulse piston, one cell from the left backbone |
| `F` | P_R | sticky piston facing east, retracted, (2,4,0) | pushes the right structure east while RB powers it |
| `A` `h` | A | sticky piston facing down, extended, (1,4,0), head on T | lifts T, then pulls C up twice |
| `B` | B | sticky piston facing down, retracted, (1,5,0) | the elevator: moves A down one cell and back |
| `G` | P_M | sticky piston facing up, retracted, (1,-2,0) | pull-type pulse piston, one cell below M |
| `Q` `$` | Q, RB | sticky piston facing east (0,4,1), redstone block (1,4,1) | RB powers A at rest; pushed to (2,4,1) it powers P_R |
| `L` `K` | lever | lever at (-2,4,0) on the wall of stone K (-2,4,1) | the input |
| `q` `k` | R_q | repeater, delay 2, at (-1,4,1), floor k | Q = the lever, 4 gt later, on both edges |
| `a` | O_a | observer watching the lever, (-1,4,0) | p0: a 2 gt pulse into X 2 gt after each flip |
| `X` `f` | X | dust (an east-west line) at (0,4,0), floor f | powers A up top directly, and A one cell lower through f |
| `b` `j` | O_b, J | observer watching O_a (-1,5,0), stone (-1,6,0) | a 2 gt pulse at t4 into R_B |
| `w` | R_B | repeater, delay 4, at (0,6,0) | stretches it to the B window t12-20, into B's quasi cell (1,6,0) |
| `o` | O | observer watching R_B, (0,5,0) | A's two dance pulses into X (t14, t22) |
| `l` `p` | P_L's observers | observer watching the lever (-3,4,0), observer watching it (-3,3,0) | a 2 gt pulse into P_L at t4 |
| `m` `g` | O_m | observer watching the bottom-right door block (2,-1,0), smooth stone (2,-2,0) | a 2 gt pulse into P_M 2 gt after that block moves |

**Budget:** 25 mechanism blocks (door 9, left 3 slime + P_L, right 6 slime + P_R, A, B, P_M, Q,
RB) + 15 control blocks:

| control | blocks |
|---|---|
| lever + its block | 2 |
| R_q + floor | 2 |
| O_a | 1 |
| X + floor | 2 |
| O_b + J | 2 |
| R_B + O | 2 |
| P_L's two observers | 2 |
| O_m + block | 2 |

### Opening

| t | what happens |
|---|---|
| 2 | O_a pulses X. A is still powered by RB, so nothing moves |
| 4 | R_q fires Q, which pushes RB east. A loses power at once and retracts, lifting T to (1,3,0). P_L reaches into its gap |
| 6 | P_L retracts and pulls the left structure west: the left column is open. T lands at (1,3,0). RB lands next to P_R |
| 7 | P_R pushes the right structure east, and its slime (2,3,0) drags T along to (2,3,0) |
| 9-11 | O_m saw the bottom-right block leave. P_M reaches up and pulls M down to (1,-1,0) |
| 12 | R_B starts the B window. B pushes A down into (1,3,0), the cell T left |
| 14-16 | O pulses X. A, powered through X's floor, extends and pulls C up into (1,2,0) |
| 20 | The window ends. B lifts A back to (1,4,0) |
| 22-24 | O pulses X again. A pulls C up into (1,3,0): **open at 24** |

### Closing

| t | what happens |
|---|---|
| 2 | O_a pulses X. A extends and pushes C from (1,3,0) down into (1,2,0) |
| 4 | The pulse ends while C is moving, so A leaves it at (1,2,0) (a spit). R_q turns off and Q pulls RB back. P_R loses power and pulls the right structure home, bringing T back to (1,3,0). P_L pushes the left structure home and spits it |
| 6 | Everything lands and RB is next to A again. O_m saw the bottom-right block return, so P_M pushes M up and spits it |
| 7 | A extends and pushes T and C down together, to (1,2,0) and (1,1,0) |
| 9 | They land: **closed at 9** |
| 12-24 | The B window fires again, but B's push fails against the extended A (gadget [blocked-push](../designs/gadgets/blocked-push.drive)). O's pulses at 14 and 22 reach an A that is already powered. Idle at 24 |

### Why it needs no hold

The rb doors before it (rb6-bmobs, 47) hold Q extended for 16 gt after the lever goes off, using
a 7-block silent hold. They need it because their close runs the whole B dance backwards before T
may come back. fc2 closes the way observer-seq's slime-free doors do: A pushes C down one cell, T
comes back, and A pushes T and C home together. Every signal comes from one chain with the same
delays on both lever edges:

* **p0** (O_a, 2 gt after each flip). On open, A is still powered and swallows it. On close, A
  pushes C down one cell, which frees T's waypoint (1,3,0).
* **Q** is the lever through one delay-2 repeater. On open, RB leaves A (A lifts T) and lands next
  to P_R 2 gt later, after T has landed, so P_R drags it. On close, RB's return makes P_R pull T
  back at once, and RB lands next to A, which pushes T and C home and stays extended.
* **The B window** comes at the same delay on both edges. On open it runs the UP dance. On close,
  A is extended (immovable), so B's push and O's pulses do nothing.

fresh-c's ideal-input sweep
([fc-sym.sched](../designs/piston-door-3x3/tracks/fresh-c/fc-sym.sched)) shows the scheme works
for any Q delay from 2 to 7, as long as the A pulse comes at least 2 ticks before Q.

### Constraints that shaped it

From fresh-c's [notes](../designs/piston-door-3x3/tracks/fresh-c/NOTES.md):

* **Q must be steady.** A pulse-driven Q spits RB, which is then placed in the block-event phase.
  P_R fires in that same phase, before T has landed, and a moving block is not dragged.
* **The lever must feed both Q's repeater and p0's observer.** RB and A's pulse dust cannot share
  a side of A, so both sit next to the lever. That is why X sits west of A, where rb6 had P_L.
* **X must be dust.** A solid block there would power Q and would not update A in its lower
  position.
* **P_L pulls from the west.** The pull-type P_L costs 3 blocks, against 5 for fc1's push-type
  piston under the walkway (pusher, extra slime, 2 chain observers, block).

Speed: open = b + 12, where b is the tick B fires. b must be at least d + 6 (T has to be aside
before B), and d, Q's delay, at least p0 + 2 = 4. So 22 is the family's floor, reached by
[fc2f](../designs/piston-door-3x3/archive/fc2f-41b.txt) (41 blocks, 22 / 9), whose B window is fed
straight off the lever.

### Is 40 the end?

Nothing found so far shortens fc2:

* `door minimize --moves all`, with plateau pairs and a 100-step neutral walk;
* `door regrow` at sizes 2 (5.4k candidates) and 3 (1.6M), and with `--any` (cut any 2 control
  blocks, add 1; 416k);
* `door grow --k 2` after cutting 3 control blocks (up to 1.57M candidates);
* `door synth`: no smaller replacement for any sub-circuit in its model;
* `door route2`: whole-net rip-up and re-route of up to 8 control blocks, two nets at once;
* riders' rider seeds (a block glued to a moving structure, then local search), and its analysis
  showing that P_M's 2 blocks are minimal in this family;
* fresh-b's flags: no gain.

fresh-c's round-2 analysis in its [notes](../designs/piston-door-3x3/tracks/fresh-c/NOTES.md)
explains why the control is pinned. X must reach A in both positions, which fixes O_a and so the
lever. Q must sit at (0,4,+-1), which fixes R_q's input. P_L only accepts 2 gt pulses or a push
from the east. The open leads are in
[the overnight report](overnight-2026-10-18.md#open-problems-and-next-ideas).

## How the mechanism evolved

* **v2, the "UP dance"** (vjeux's session, 151 -> 115). The side columns ride slime backbones
  pulled by pistons on NOT lines, M is pulled down, A lifts T and P_T pulls it aside, and C climbs
  on A riding B. Most of its blocks were torches, NOT gates and delay lines.
* **rb1-rb6** (73 -> 48). The side structures are pushed open by pistons that are off at rest. One
  redstone block shuttled by Q switches A off, then P_R on, and the right structure drags T, so P_T
  disappears. A 7-block silent hold keeps Q out through the closing dance.
* **rb6-bmobs** (47). P_M is pulse-driven, fired by an observer watching the bottom-right door
  block.
* **fc1** (42). Symmetric control: no hold.
* **fc2** (40). P_L becomes a pull-type pulse piston west of its backbone, fired by two observers
  off the lever.

Every step is in the [archive](../designs/piston-door-3x3/archive/README.md) with a
[video](../designs/piston-door-3x3/archive/VIDEOS.md).

## What the local search finds (and what it does not)

* **Re-synthesized nets are shorter.** Nets rebuilt from their terminals (renet) beat nets laid one
  connection at a time: v2 113 -> 109, and v2u2 96 -> 88 once a repeater dissolved into dust.
* **Dust + its floor -> one block** (swap). An observer's or repeater's output strongly powers a
  plain block, which feeds the next part directly: sculk 48 -> 46 and 48 -> 43, gravity 49 -> 46,
  fast-60 -> 57.
* **Floors next to moving slime must not be dragged**, so they are glazed terracotta. The router
  knows this.
* **It does not invent mechanisms.** fc2 40, mp8 43, rb6-bmobs 47, slime-free 48, st43-bmobs 42 and
  g10 45 are all local minima of the search: removals of 1-3 blocks, swaps, moves, substitutions,
  renets, and neutral-edit plateaus. What slack remains is in mechanism choices.
* **Exact re-synthesis found wiring slack it missed.** `door resynth` clears every connected group
  of a few wiring blocks (2-4 by default) and tries every smaller circuit in its place: g10 45 -> 42
  (the gravity record) and slime-free-combo 48 -> 46.

## Tools

The `mctas door` commands are listed in the [README](../README.md#tools). The workflow behind the
records:

1. Validate a mechanism with ideal inputs, either redstone blocks placed by a `door drive` script
   or a `door ideal` schedule.
2. Wire it, by hand or with `door route`.
3. Run `door verify --extra-offsets 28`.
4. Shrink it with `door minimize`, `door grow` and `door regrow`.

Proven circuits go in the gadget library (`designs/gadgets/`, run them all with
`mctas door drive --quiet designs/gadgets/*.drive`). Mechanics read from the source are in
[mechanics.md](mechanics.md).

## Tileable doors

`door verify <module> --tile <spec>` judges copies of one module door run by ONE lever (the other
copies hold the module's `relay:` block in the lever's cell, so the module carries its own bus),
with every rule checked on every opening:

* `period:P:N`: an opening every P blocks along a wall. Best: **45 blocks per opening at P = 8**
  ([b38h-obs8b](../designs/piston-door-3x3/tracks/tile/b38h-obs8b.txt)): b38 with its note block
  turned into a hopper, plus 7 observers that watch the lever's cell and carry each lever edge to
  the next copy (b38 reacts to edges). Copy k opens 14k ticks after copy 0.
* `vperiod:Q:N`: a door every Q blocks up. Best: **49 per opening at Q = 9**
  ([b38h-vobs9m](../designs/piston-door-3x3/tracks/tile/b38h-vobs9m.txt)), an observer tower beside
  the openings.
* `wide:N` (openings side by side, one 3 x 3N opening) and `stack:N` (a tall door): none yet. In a
  periodic wide row, the space above and the space below the openings share no cell (nothing may
  stand in front of or behind any opening), so the bottom row's pistons can only be reached
  through the door blocks or a shared end piece (`--tile-extra`). The mechanism (each column =
  b38's center column) works with ideal inputs.

Both bests' 3-copy arrangements pass `door vanilla-check`. Rankings: `door tile-board`
(designs with a `tiling:` header). Details: [tracks/tile/NOTES.md](../designs/piston-door-3x3/tracks/tile/NOTES.md).

## Solid record history

| blocks | design (archive) | how |
|---|---|---|
| 151 | piston-door-3x3-v2-151b | first valid design (v1 used a slime block as the top-middle door block, which the verifier now rejects) |
| 147 -> 115 | piston-door-3x3-v2-147b ... -115b | vjeux's session: router-laid wires, unified late chain, greedy removals (v2-min) |
| 113, 109 | search-tool-v2-min-113b, -109b | local search: a reroute, then net re-synthesis |
| 96 | piston-door-3x3-v2u2-96b | sides pushed out from under the walkway, redstone blocks riding the side structures |
| 91 | mechanics-ta-91b | A's torch next to the observer column |
| 87 | piston-door-3x3-v3c-87b | side pistons high, redstone blocks on the backbones |
| 73 | piston-door-3x3-rb1-73b | movable power: one shuttled redstone block, T dragged by the right structure |
| 57 | piston-door-3x3-rb2-57b | rb1 with short nets |
| 53, 51, 50 | mechanics-rb2-hold-53b, -51b, mechanics-rb5-bchain-50b | short hold, pulse bridge, rb5's silent hold |
| 48 | mechanics-rb5-hold-48b, piston-door-3x3-rb6-48b | the silent hold in 7 blocks |
| 47 | rb6-bmobs-47b | observer-fired bottom-middle piston |
| 46 | piston-door-3x3-fresh-b-mp6-46b | mirrored pump |
| 42 | fc1-42b | symmetric control |
| 40 | fc2-40b | pull-type left piston |
