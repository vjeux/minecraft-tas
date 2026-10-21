# History of the 3x3 piston door

What happened: the record progressions, the door families and how they work, the discoveries and
the dead ends. The rules are in [rules.md](rules.md); the live rankings in
[LEADERBOARD.md](../designs/piston-door-3x3/LEADERBOARD.md); every archived design with its video
in the [archive catalog](../designs/piston-door-3x3/archive/README.md); each agent track's raw
notebook in `designs/piston-door-3x3/tracks/<track>/NOTES.md` ([index](../designs/piston-door-3x3/tracks/README.md)).
Times are UTC.

## The boards (state on 2026-10-18 at 18:00)

| board | best | blocks | open / close | idea (track) |
|---|---|---|---|---|
| **Solid** | [sf-holdA-b38-38b](../designs/piston-door-3x3/archive/sf-holdA-b38-38b.txt) | **38** | 36 / 32 | mech-synth's 19-block slime-free mechanism with A held at rest, X's three pulses toggling both top corners, the lever on an observer next to a note block (sf-holdA); also the slime-free record |
| Solid, no slime or honey | sf-holdA-b38-38b | 38 | 36 / 32 | the same door |
| Solid, flat (one z layer), no slime or honey | [lamp-sweep-flat43-43b](../designs/piston-door-3x3/archive/lamp-sweep-flat43-43b.txt) | 43 | 46 / 44 | the 19-block mechanism in one plane (flat's 49), re-synthesized with a redstone lamp (lamp-sweep) |
| Solid, in a 3-thick wall | [fc2-40b](../designs/piston-door-3x3/archive/fc2-40b.txt) | 40 | 24 / 9 | symmetric control (fresh-c) |
| Solid, on the ground | - | - | - | no entry yet |
| Solid, smallest box | [flat-b-v7-44b](../designs/piston-door-3x3/archive/flat-b-v7-44b.txt) | 44 | 30 / 26 | the 19-block mechanism upside down in one plane (flat-b) |
| Sculk | [st43-bmobs-42b](../designs/piston-door-3x3/archive/st43-bmobs-42b.txt) | 42 | 22 / 38 | the rb family with a sculk sensor as Q's hold (top-control) |
| Not rotation-safe | [sculk2-s4-36b](../designs/piston-door-3x3/archive/sculk2-s4-36b.txt) | 36 | 20 / 37 | st43 with a calibrated sculk sensor replacing a 5-block chain; works facing north or south only (sculk-2) |
| Gravity | [gravity2-g33-33b](../designs/piston-door-3x3/archive/gravity2-g33-33b.txt) | 33 | 5 / 16 | a cheaper sand drop tower (gravity-2) |
| Fastest open + close, solid | [speed-2-c1-112b](../designs/piston-door-3x3/archive/speed-2-c1-112b.txt) | 112 | 9 / 7 | an odd pulse lets Q spit A' at k5; the lever net re-laid over the top under the column rule (speed-2) |
| Fastest close, solid | [fast-clean-a1c-107b](../designs/piston-door-3x3/archive/fast-clean-a1c-107b.txt) | 107 | 10 / 7 | lamp-sweep's a1-107 with its lever net re-laid over the top (fast-clean) |
| Fastest open, solid | [tick-door-open4-229b](../designs/piston-door-3x3/archive/tick-door-open4-229b.txt) | 229 | **4** / 15 | the proven floor: the open-4 machine wired to a lever (tick-door) |
| Fastest open + close, any category | [gravity-g8-48b](../designs/piston-door-3x3/archive/gravity-g8-48b.txt) | 48 | 5 / 11 | the drop tower with a pulse-driven middle piston (gravity) |

Every solid record, filmed by one camera:

[![every solid record](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.jpg)](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.mp4)

![the record of each board over the night](https://github.com/vjeux/minecraft-tas/releases/download/videos/night-timeline.png)

Other progression videos:
[slime-free](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-progression.mp4),
[flat](https://github.com/vjeux/minecraft-tas/releases/download/videos/flat-progression.mp4),
[sculk](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk-progression.mp4) and
[gravity](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-progression.mp4).

## Before the night: v1 and v2 (vjeux's session, 151 -> 115)

* **v1** (125, then 121) used a slime block as the top-middle door block, which is not a real door;
  the judge now rejects it (`archive/piston-door-3x3-v1-invalid.txt`).
* **v2, the "UP dance"** (151): the first valid design, all stone. The side columns ride slime
  backbones pulled by pistons on NOT lines, the bottom-middle M is pulled down, the center piston A
  lifts the top-middle T and a piston P_T pulls it aside, and the center C climbs on A riding the
  elevator piston B. Most of its blocks were torches, NOT gates and delay lines.
* vjeux's session took it to 115 with router-laid wires, a unified late chain and greedy removals
  (v2-min). 94 of its 115 blocks were wiring.

## The night of 2026-10-18

At 05:14 a coordinator agent started the first of the night's agent tracks on two dev servers (up
to 10 at once; 37 of them left a notebook in `tracks/`). By 08:10 the solid record was **40**
(fresh-c's fc2); at 15:15 **38** (sf-holdA's b38).

### How 115 became 38

| time | blocks | design | track | key idea |
|---|---|---|---|---|
| start | 115 | [v2-115b](../designs/piston-door-3x3/archive/piston-door-3x3-v2-115b.txt) | vjeux's session | v2 "UP dance"; 94 of its 115 blocks are wiring |
| 05:35 | 109 | [search-tool-v2-min-109b](../designs/piston-door-3x3/archive/search-tool-v2-min-109b.txt) | search-tool | local search: a reroute, then the lever net re-synthesized |
| 05:48 | **96** | [v2u2-96b](../designs/piston-door-3x3/archive/piston-door-3x3-v2u2-96b.txt) | center-mech | **sides pushed out** by pistons that are off at rest (no NOT lines) |
| 05:48-50 | 91, 88, 87 | [mechanics-ta-91b](../designs/piston-door-3x3/archive/mechanics-ta-91b.txt), [v3c-87b](../designs/piston-door-3x3/archive/piston-door-3x3-v3c-87b.txt) | mechanics, search-tool, top-control | A's torch next to its observer; side pistons high |
| 05:51 | **73** | [rb1-73b](../designs/piston-door-3x3/archive/piston-door-3x3-rb1-73b.txt) | redstone-block | **movable power**: one shuttled redstone block powers A, then the right piston, whose structure drags T aside (no P_T) |
| 05:53 | **57** | [rb2-57b](../designs/piston-door-3x3/archive/piston-door-3x3-rb2-57b.txt) | redstone-block | rb1 with short nets |
| 06:00-10 | 53, 51, 50, 48 | [mechanics-rb5-hold-48b](../designs/piston-door-3x3/archive/mechanics-rb5-hold-48b.txt), [rb6-48b](../designs/piston-door-3x3/archive/piston-door-3x3-rb6-48b.txt) | mechanics, redstone-block | Q's hold shrunk to a 7-block BUD "silent hold" |
| 06:44 | **47** | [rb6-bmobs-47b](../designs/piston-door-3x3/archive/rb6-bmobs-47b.txt) | top-control | **observer-fired bottom-middle** piston, retracted at rest |
| 08:08 | 46 | [mp6-46b](../designs/piston-door-3x3/archive/piston-door-3x3-fresh-b-mp6-46b.txt) | fresh-b | new family: **mirrored pump** |
| 08:09 | 42 | [fc1-42b](../designs/piston-door-3x3/archive/fc1-42b.txt) | fresh-c | new family: **symmetric control**, no hold |
| 08:10 | **40** | [fc2-40b](../designs/piston-door-3x3/archive/fc2-40b.txt) | fresh-c | fc1 with a pull-type pulse piston for the left side |
| 10:03 | 40, tie | [riders-fx1-40b](../designs/piston-door-3x3/archive/riders-fx1-40b.txt) | riders | fc2's left piston toggled by X's 3 pulses per flip; its 2 observers become 2 slimes |
| 11:25 | 40, tie | [m05-21b-sym-40b](../designs/piston-door-3x3/archive/m05-21b-sym-40b.txt) | wire21 | new family: mech-synth's 21-block pulse-only mechanism |
| 15:15 | 39 | [sf-holdA-b39-39b](../designs/piston-door-3x3/archive/sf-holdA-b39-39b.txt) | sf-holdA | the 19-block slime-free mechanism, both top corners toggled by X's 3 pulses (the doorway bounces before it settles), the B window and the T piston on one observer chain |
| 15:15 | **38** | [sf-holdA-b38-38b](../designs/piston-door-3x3/archive/sf-holdA-b38-38b.txt) | sf-holdA | b39 with the lever on the observer O next to a note block: a zero-delay relay that keeps power out of the T piston's quasi cell |

* **05:14-06:10, the wiring collapses (115 -> 48).** Search cleaned up v2's wires, then pushed
  sides and movable power deleted whole subsystems: the side NOT lines, P_T, A's nets.
* **06:10-08:08, the plateau.** Every search called 47-48 a local minimum, and Q's 7-block hold
  could not be made cheaper.
* **08:08-08:14, two new families.** The mirrored pump broke 47, symmetric control dropped the
  hold: 46, 42, 40.
* **08:14-15:00, no 39.** Instead, three different 40s that spend the blocks differently: fc2 25
  mechanism + 15 control, fx1 27 + 13, m05 21 + 19.
* **15:15, the slime-free mechanism wins.** sf-holdA held A extended at rest on mech-synth's
  19-block mechanism (fresh-c's symmetric close without slime) and reused X's three pulses: 39,
  then 38 (archived together), which is also the slime-free record. Searches on b38 found nothing
  smaller (`minimize --plateau` with 30k neutral edits, `resynth` up to 5 blocks with note blocks,
  `regrow --size 3` 1.42M candidates).

The full solid progression from 151 (with the v2 steps) is
[archive/records.tsv](../designs/piston-door-3x3/archive/records.tsv), shown in the catalog.

### The other boards

| board | path | how |
|---|---|---|
| Slime-free | 89 (observer-seq s1, the first) -> 58 (s6) -> 53, 50 (logic-min) -> 48 (slime-free's combo) -> 45 (09:18) -> 44 (09:36) -> 43 (11:56) -> 42 (13:05) -> 41 (13:43) -> 38 (15:15, sf-holdA) | observer-seq: event-driven sequencing; logic-min: each side column on 3 pulse-fired sticky pistons; s7: fresh-c's close without slime (A held by a shuttled redstone block, P_T pulse-driven); s7q: `door regrow` made Q pulse-driven; w43/w42: the 19-block mechanism on one pulse train; w41: re-synthesized with a lamp; b38: the same mechanism with A held |
| Flat (opened 10:56 by vjeux) | 60 (11:36) -> 57 -> 55 -> 54 (12:10) -> 49 -> 48 (12:47) -> 47 (12:49) -> 46 -> 45 -> 44 -> 43 | flat: the 19-block mechanism in one plane, `minimize`, R_B fed by a dust under a powered block, `synth`/`resynth` on the late chains (49). flat-b: the mechanism upside down, the lever hung under the observer that watches it (47), then 44. lamp-sweep: flat's 49 re-synthesized with a lamp (43) |
| Sculk | 50, 48 (center-mech) -> 43 (search-tool) -> 42 (st43, top-control) -> 38 (12:46, sculk-2) -> 36 (13:30) -> 36 rotation-safe (sculk-ew) | a sculk sensor as Q's hold (top-control, center-mech); sculk-2: one calibrated sculk sensor (active 10 gt) replaces st43's 5-block lever-to-R_B chain (38), then a moved pusher whose retracting base blocks an early pull (36). The rotation check (15:51) moved sculk-2's doors to the Not rotation-safe board; sculk-ew found why they broke facing east/west (the landing block's update order decides whether Q or A fires first: gadget `landing-race`) and fixed all four with one repeater delay 1 -> 2, no extra block (s4r 36, all 8 orientations) |
| Gravity | 51 (g4) -> 49 (g6) -> 46, 45 (search-tool, g10) -> 42 (09:47, `door resynth`) -> 36 -> 35 -> 33 (15:36, gravity-2) | the sand drop tower: sand side columns pushed open by the doorway-floor pistons, the sand middle column dropping into a shaft on a collapsing 3-piston tower |
| Fastest solid | 16 / 17 (rb8, mechanics-fast-57b) -> mp8 16 / 16 -> w6 12 / 7 at 161 blocks -> s4 12 / 7 at 109 -> o2 11 / 7 at 137 -> o7 11 / 7 at 119 -> speed-2 a1 10 / 7 at 125 -> lamp-sweep a1-107 10 / 7 at 107 -> speed-2 n2 9 / 7 at 113 -> c1 112, c2 109 (9 / 7) | a one-repeater silent hold; open 11 from a 0-tick inside one block-event phase; open 10 from an odd-tick pulse (a repeater reading a moved redstone block + a note block + an observer); open 9 with Q spitting A' at k5 off an odd pulse. After the 16:40 rule every fast door had to re-lay the lever net's staircase that crossed in front of the door (fast-clean, speed-2's c1/c2). **Open 4**, the proven floor, is wired to a lever in tick-door's w14 (229 blocks, closes in 15) |

### Rule changes during the night

* **09:48** floating blocks rejected (`canSurvive`); **12:03** randomly ticking blocks rejected
  (fresh-d's unwaxed copper bulb; its files are marked invalid, the waxed 45 stays).
* **10:56** new board: solid, flat (one z layer), no slime or honey.
* **15:07** bouncing mid-cycle allowed (only the settled states are judged).
* **15:51** the rotation check: the boards rank rotation-safe designs; the others get their own
  board. Re-judged: solid 38, gravity 33, sculk-2's doors not rotation-safe (they work facing north
  or south only; the sculk-ew track builds east/west variants).
* **16:40** nothing may stand in front of or behind the door, at any distance (designs judged
  before keep their verdicts for now). The routers and searches learned to keep those columns clear.
* **After 17:00** three shape boards (in a 3-thick wall, on the ground, smallest box: the shapes
  track's swept shapes and ground check), the tile judge for tileable doors (one module, one lever:
  the tile track's b38h-obs8 opens a door every 8 blocks at 46 blocks per opening), and east/west
  variants for doors that only work facing one way (`door turn`, `door-axis: z`); sculk-ew instead
  made sculk-2's doors work in all 8 orientations with one repeater delay change (s4r, 36).
  pareto-mid filled the solid frontier's middle with new delay elements (a log and persistent leaves,
  a redstone block and a lamp: gadgets `log-leaves`, `rb-lamp`): 43 at 15 / 17, 44 at 14 / 16, 46 at
  15 / 13 and 14 / 14; the exotic track then made it 45 at 14 / 14 with an observer watching an iron
  trapdoor (trapdoors, fence gates, skulls, rails, dispensers, crafters and scaffolding are now in
  the search palettes). A new category, **entity** (minecarts, armor stands, boats... declared in
  the design and judged exactly back at rest after each close), got its own board, "Entities
  allowed" (the entities track).

## The door families

| family | best | idea |
|---|---|---|
| UP dance (v2) | v2-min 115 | slime side backbones pulled by pistons on NOT lines, M pulled down, P_T pulls T aside, C climbs on A riding B |
| rb, movable power | rb6-bmobs 47; riders-r3 43 | pushed sides; a shuttled redstone block turns A off, then P_R on; the right structure drags T. Costs a hold for Q through the closing dance |
| Mirrored pump (fresh-b) | mp8 43, opens in 16 | a pump under the door; every piston fires once per edge, triggered by **flags** (blocks glued to moving structures, watched by observers) |
| Symmetric control (fresh-c) | fc2 40, fx1 40 | the same signals at the same delays on both edges. On close, A spits C down, T comes back, A pushes both home, and the B window pushes uselessly against the extended A (walk-through below) |
| Pulse-only 21 (mech-synth, wire21) | m05 40 | every piston retracted at rest and fired by 2 gt pulses, one 9-pulse train on both edges (P_L, A, P_T, B on, A, B off, A, M, P_R): no shuttle, no hold, no torch. T leaves north above the walkway |
| Slime-free 19 (mech-synth, wire19sf, flat, flat-b, lamp-sweep, sf-holdA) | sf-holdA b38 38; flat43 43 | every edge block on its own pull-type sticky piston, plus the A/B elevator and a T piston: 19 mechanism blocks, the floor of this family. One piston must fire after A's last pull: M at the bottom (t >= 22 on close in w43), or, with the elevator below the door (flat-b), the top pullers. sf-holdA holds A extended at rest (fresh-c's symmetric close), so M may fire early |
| Slime-free shuttle (slime-free, fresh-d) | s7q 44, fresh-d s4 45 | fc2's close without slime: A held by a shuttled redstone block, side columns on pulse pistons |
| Gravity drop tower (gravity, synth, gravity-2) | g33 33, opens in 5 | sand sides pushed open by the doorway-floor pistons; the sand middle column drops 3 cells into a shaft on a collapsing 3-piston tower, which lifts it back |
| Sculk (top-control, search-tool, sculk-2) | st43 42; sculk2-s4 36 (not rotation-safe) | the rb family with a sculk sensor as Q's hold; a calibrated sensor (active 10 gt) also replaces the B trigger. Own board: footsteps trigger sensors |
| Speed (speed, speed-2, lamp-sweep) | a1-107 107, 10 / 7 | T leaves first, a pusher shoves a sticky puller into T's exit cell, two quick pulls take C out; F waits armed (a BUD) to close in 7 |

## fc2 (40 blocks), the symmetric-control door

The solid record from 08:10 to 14:40. Open 24 ticks, close 9 (idle at 26 / 24). Below, t is game
ticks after the lever flip, read from `mctas door trace designs/piston-door-3x3/archive/fc2-40b.txt --close`.

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
RB) + 15 control blocks (lever + its block 2, R_q + floor 2, O_a 1, X + floor 2, O_b + J 2, R_B + O
2, P_L's two observers 2, O_m + block 2).

**Opening**

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

**Closing**

| t | what happens |
|---|---|
| 2 | O_a pulses X. A extends and pushes C from (1,3,0) down into (1,2,0) |
| 4 | The pulse ends while C is moving, so A leaves it at (1,2,0) (a spit). R_q turns off and Q pulls RB back. P_R loses power and pulls the right structure home, bringing T back to (1,3,0). P_L pushes the left structure home and spits it |
| 6 | Everything lands and RB is next to A again. O_m saw the bottom-right block return, so P_M pushes M up and spits it |
| 7 | A extends and pushes T and C down together, to (1,2,0) and (1,1,0) |
| 9 | They land: **closed at 9** |
| 12-24 | The B window fires again, but B's push fails against the extended A (gadget [blocked-push](../designs/gadgets/blocked-push.drive)). O's pulses at 14 and 22 reach an A that is already powered. Idle at 24 |

**Why it needs no hold.** The rb doors before it (rb6-bmobs, 47) hold Q extended for 16 gt after
the lever goes off, using a 7-block silent hold, because their close runs the whole B dance
backwards before T may come back. fc2 closes the way observer-seq's slime-free doors do: A pushes C
down one cell, T comes back, and A pushes T and C home together. Every signal comes from one chain
with the same delays on both lever edges:

* **p0** (O_a, 2 gt after each flip). On open, A is still powered and swallows it. On close, A
  pushes C down one cell, which frees T's waypoint (1,3,0).
* **Q** is the lever through one delay-2 repeater. On open, RB leaves A (A lifts T) and lands next
  to P_R 2 gt later, after T has landed, so P_R drags it. On close, RB's return makes P_R pull T
  back at once, and RB lands next to A, which pushes T and C home and stays extended.
* **The B window** comes at the same delay on both edges. On open it runs the UP dance. On close, A
  is extended (immovable), so B's push and O's pulses do nothing.

fresh-c's ideal-input sweep ([fc-sym.sched](../designs/piston-door-3x3/tracks/fresh-c/fc-sym.sched))
shows the scheme works for any Q delay from 2 to 7, as long as the A pulse comes at least 2 ticks
before Q.

**Constraints that shaped it** (fresh-c's [notes](../designs/piston-door-3x3/tracks/fresh-c/NOTES.md)):

* **Q must be steady.** A pulse-driven Q spits RB, which is then placed in the block-event phase.
  P_R fires in that same phase, before T has landed, and a moving block is not dragged.
* **The lever must feed both Q's repeater and p0's observer.** RB and A's pulse dust cannot share a
  side of A, so both sit next to the lever. That is why X sits west of A, where rb6 had P_L.
* **X must be dust.** A solid block there would power Q and would not update A in its lower
  position.
* **P_L pulls from the west.** The pull-type P_L costs 3 blocks, against 5 for fc1's push-type
  piston under the walkway (pusher, extra slime, 2 chain observers, block).

**Speed:** open = b + 12, where b is the tick B fires. b must be at least d + 6 (T has to be aside
before B), and d, Q's delay, at least p0 + 2 = 4. So 22 is the family's floor, reached by
[fc2f](../designs/piston-door-3x3/archive/fc2f-41b.txt) (41 blocks, 22 / 9), whose B window is fed
straight off the lever.

## Mechanics discoveries

Read in the 26.3 source, each proven by a [gadget](../designs/gadgets/) or a judged door; details in
[mechanics.md](mechanics.md).

* **Pulse-driven pistons** (`pulse-toggle`, `spit`). A retracted sticky piston one cell from its
  block, given one 2 gt pulse per edge, is a toggle: on open it reaches across the gap and pulls, on
  close it pushes and lets go. **Three pulses toggle it too** (fx1, m05, sf-holdA): one net move,
  timed by the last pulse, so a piston can reuse a source that fires three times per flip.
* **Movable power.** A shuttled redstone block switches one piston off at once and another on 2 gt
  later (rb1); a redstone block or an observer riding a structure is a timing element (riders).
* **Asymmetry for free.** A lamp lights at once and goes dark 4 gt after its power ends (m05's late
  P_R pulse). A waxed copper bulb toggles on rising edges only, so an observer on it pulses once per
  edge (`bulb-pulse`). A push-type and a pull-type pulse piston fed the same pulse act in opposite
  orders on the two edges (`order-flip`). A weakly powered note block next to the lever is a
  zero-delay watch target that conducts nothing (sf-holdA's 38).
* **Quasi-connectivity.** A piston is quasi-powered through the head of the piston above it, so
  fc2's X dust needs no conducting floor (`head-qc`; fc2-40b-2 stands it on glass). A quasi-powered
  piston waits for an update (`silent-hold`), but a piston head forwards neighbor updates to its
  base, which is why fc2's Q cannot wait silently next to A (q-free).
* **Observer chains** cost 1 block per 2 gt. The tick at which a chain can fire a given cell is
  fixed mod 4 (aim at the piston's quasi cell to shift it by 2), and an observer watching a
  pulse-pulled door block fires 2 gt earlier on close than on open (wire19sf). A dust under a
  strongly powered block lights (`dust-below`, flat 54).
* **In one plane** (flat): repeaters cannot be locked (their sides face out of the plane), and a
  lever powers its neighbors, so it must stay off every piston and quasi cell.
* **Speed.** A piston 0-ticks when the redstone block powering it is shoved away in the same
  block-event phase (o2: open 11); odd ticks come only from pistons (speed-2's open 10, then 9). The
  tick-study proved the floors for solid doors, **open >= 4 and close >= 1**, and built a whole
  opening clear at tick 4 that closes in 3 with ideal inputs (`open-4`, `open-4-close3`); the
  tick-door track wired it to a lever: tick-door-open4-229b opens in 4 (closes in 15, 229 blocks).
* **Sculk** (sculk-2). A vibration reaches a sensor max(1, floor(distance)) ticks after the event; a
  sensor stays active 30 gt (a calibrated one 10 gt), then deaf for 10. A lever flip and a piston's
  move are vibrations (a blocked extension is not); observers, repeaters and dust make none. A
  player standing on a sensor triggers it (vanilla-check). Wool hides vibrations.

## What the local search finds (and what it does not)

* **Re-synthesized nets are shorter.** Nets rebuilt from their terminals (renet) beat nets laid one
  connection at a time: v2 113 -> 109, and v2u2 96 -> 88 once a repeater dissolved into dust.
* **Dust + its floor -> one block** (swap). An observer's or repeater's output strongly powers a
  plain block, which feeds the next part directly: sculk 48 -> 46 and 48 -> 43, gravity 49 -> 46,
  fast-60 -> 57.
* **Floors next to moving slime must not be dragged**, so they are glazed terracotta. The router
  knows this.
* **It does not invent mechanisms.** fc2 40, mp8 43, rb6-bmobs 47, slime-free 48, st43-bmobs 42 and
  g10 45 were all local minima of the search: removals of 1-3 blocks, swaps, moves, substitutions,
  renets, and neutral-edit plateaus. What slack remains is in mechanism choices.
* **Exact re-synthesis found wiring slack it missed.** `door resynth` clears every connected group
  of a few wiring blocks and tries every smaller circuit in its place: g10 45 -> 42 (gravity),
  slime-free-combo 48 -> 46, and, with lamps, copper bulbs, note blocks and targets in the palette
  (lamp-sweep), flat 49 -> 43, w42 -> 41 and the fastest solid door 123 -> 107.
* **Mechanism synthesis did.** `door mech-synth` found the 21-block pulse-only mechanism (m05) and
  the 19-block slime-free one (w42, flat, flat-b, b38). `door mech-wire` wires mechanisms
  automatically but ~12 blocks worse than by hand (53 vs m05's 40).

## Correctness

* **The judge got stricter** (09:48 floating blocks, 12:03 random ticks, 16:40 nothing in front of
  or behind the door) and gained the rotation check (15:51). Rules: [rules.md](rules.md).
* **Simulator bugs fixed early**: torch burnout memory leaked across sandbox reuse (everything was
  re-judged, cache v2), and the judge did not wait for falling sand.
* **Same as the real game.** `door vanilla-check` runs the official 26.3 server jar unmodified; a
  Rust bot clicks the lever and the region files are read back every tick. At 09:30: the 16 bests
  (fc2: 4,832 ticks) identical at 32 positions x 3 cycles plus a real-time cycle, every archived
  design at 4 positions, every branch best at 8, plus 140 random contraptions (`vanilla-fuzz`). At
  13:29 the newer leaders too: fx1 (5,024 ticks), m05 (7,904), w42 (5,408), flat-b 47 (6,368), g42
  (3,758), sculk 38 (9,536) and the fastest solid o7 (2,912). No simulator divergence; each apparent
  one was a harness bug. In real play a player standing on a sculk sensor triggers it, which breaks
  rb2s2-50b and rb2-sculk-50b. The whole gadget library behaves identically too
  ([mechanics.md](mechanics.md#11-vanilla-confirmation)).
* **Buildable in survival** (`door buildable`, results in the
  [archive catalog](../designs/piston-door-3x3/archive/README.md), scripts in
  `tracks/buildable/scripts/`). The judge pastes every block raw, so its rest state could be one a
  player never reaches by placing blocks one at a time. The tool searches for an order of real
  player placements (an item click on a visible face, with the look direction that gives the
  orientation; the world settles after every action; temporary dirt blocks placed and broken later,
  needed only because the judge's world is a void) that ends exactly in the judge's rest state, then
  runs the judge's cycles from the built state. Every board leader checked (fc2, fx1, m05, mp8, s7q,
  st43, g42, g10, the speed doors, w42, flat 60, flat-b 47, sculk 38, sf-holdA b38) is buildable at
  32 of 32 positions, but none in the naive order (mechanism, pistons, wiring, observers, power
  last):
  * observers see their neighbours being placed, and temporary blocks being broken: an observer
    that must face a cell can only be placed against a temporary block in that cell, and breaking it
    pulses the piston behind (fc2 step 33/40, mp8, st43), or placing the lever is a change the
    observer next to it sees and the half-built machine runs (fx1 step 39/40);
  * sand needs its support first (g42 step 14/42);
  * some components have to go in while a face giving their orientation is still visible (the speed
    doors' floor lever and observers facing north);
  * riders-r3 and sculk 38 are built with the lever ON and switched off at the end, which runs one
    settling cycle;
  * the self-test, two observers facing each other (quiet in the judge's rest state, a clock as soon
    as a player places the second), is correctly NOT buildable.
* **The copper bulb hazard** (fresh-d-s3-47b, fresh-d-s4-45b): the unwaxed bulb watched by an
  observer oxidizes (random ticks), each stage is a block change the observer sees, and the door
  fires on its own: `verify --cycles 40` BROKEN at cycle 20, and the build check failed at one
  position with the bulb turned `exposed_copper_bulb`. The waxed version (same 45 blocks) works for
  40 cycles at all 32 positions and is buildable. This led to the random-tick rule.
* **Built in the real game** (`door vanilla-build`). A bot player replays the build scripts on the
  unmodified server (item in hand, the script's look, sneaking, a right-click on the given face),
  compared with the simulator after every action. Every leader above plus w43, flat-v2-54b and
  sculk2-s1-38b PASSES: identical after every action, built world = rest state, cycles identical
  tick for tick (4 positions; fc2-40b-2 at 32; also built at real-time 20 tps). Findings: a player
  standing by st43's sensor makes it fire one extra time after step 26; 12% of clicks have no
  standing spot on their exact view ray ([notes](../designs/piston-door-3x3/tracks/vanilla-build/NOTES.md)).

## Dead ends and open problems

* **Below 38.** b38 is a local minimum of `minimize --plateau`, `resynth` up to 5 blocks (with note
  blocks) and `regrow --size 3`. Earlier, fc2 and its variants were local minima of every search:
  `minimize` with plateaus (255 neutral designs at depth 2), `regrow` including `--any` (24.3M size-3
  candidates screened, none passed), `grow` after cutting 3, `synth`, `resynth`, `route2` (whole-net
  rip-up of up to 8 control blocks, two nets at once) and riders' rider seeds; m05 too (`regrow
  --size 3`: 2.4M; no P_R driver under 6 blocks). By analysis, each fc2 control pair is pinned
  (fresh-c, fresh-e: X must reach A in both positions, which fixes O_a and so the lever; Q must sit
  at (0,4,+-1); P_L only accepts 2 gt pulses or a push from the east), P_M's 2 blocks are minimal in
  fc2 and fx1 (riders), and a "free" Q delay fails on close because Q's head passes the A pulse's
  update to Q (q-free). mech-synth: with fc2-style slime sides and M fixed, no mechanism below 21
  blocks (19,967 combinations); without slime, 19 is the floor of its family. Leads: its catalogs
  (12 pulse-only 21s, 396 symmetric 22s, two without the B piston) ranked by wiring cost, odd pulse
  counts as a search move, one pulse source shared by P_L and P_M (each works at any tick 0..30).
* **Mechanism + wiring jointly** (mech-wire): its router wires ~12-14 blocks worse than hand wiring,
  so its ranking is an upper bound, not a record finder (yet).
* **Slime-free before b38.** w43 was a local minimum (`minimize --plateau`, `regrow --size 3` 2.1M,
  `synth` for M's trigger up to 6 blocks). The crux was M, which must move at t >= 22 on close (after
  A's last pull); a lamp only delays 4 gt, and the redstone block south of A costs 46 or more.
  sf-holdA's held A removed that constraint.
* **Flat.** For the flat track's layout (the 19-block mechanism with the elevator on top) the late
  left chain is geometrically minimal (9 observers), and every flat slime-free pulse-only mechanism
  mech-synth found needs one late piston (66k combinations, 103 symmetric): the others need a piston
  riding a slime column.
* **Faster.** The wired solid doors open in 9 and close in 7; tick-door's w14 opens in 4, the proven
  floor, but closes in 15 at 229 blocks; open 8 works with scripted inputs (speed-2). An
  ideal-input machine does 4 / 3 (close floor: 1). The symmetric family's open floor is 22 (fc2f).

### Below 72: flat, slime-free, FULL seamless + hallway (2026-10-18 evening)

**Community existence (vjeux, 00:00Z): flat doors 7 wide x 8 tall = 56 and 7 wide x 9 tall = 63
exist** (no designs; never look them up). So every "7 wide is blocked" verdict below is an
artifact of our bounds, not of the box: look for what the bounds exclude.

**Caveat on every satwire verdict below (sweep56, 01:15Z):** a satwire UNSAT is relative to the
ideal plan it was given; with families' plans even e89b's own mechanism is UNSAT in e89b's own box.
Treat "satwire UNSAT" lines as "this plan can't be wired", not "this mechanism can't".
**8x8-131 and 8x8-135 (v3) fit 7 wide x 8 tall** (every mechanism cell stays in x -2..4 over a
full cycle): the catalog filed them under 8x8, so its 7x8 list is incomplete.

Record: e89b, 8x9x1 = 72, 49 blocks (flat-seam). What is known to fail, and under which bounds
(every "none" is bounded; tricks that may break a bound are in
[mechanics.md](mechanics.md#wiring-tricks-for-tight-flat-boxes-found-2026-10-18-evening-below-72-hunt)):

* **e89b is minimal for its own mechanism and exact timing**: `door satwire` re-found it from
  scratch and proved 28 wiring blocks UNSAT in 8x9 (palette block, dust, rep, obs, torch). Open:
  other components, off-plane lever, retimed schedules.
* **5-tall strips (11x5 = 55, 12x5 = 60) cannot be wired with fixed parts** (strip, "Lemma"):
  every row mechanism fires twice from the inner door column and once from the ring column beside
  it, and the corner cell would need both roles; 0 rows up to 12 wide, <= 7 pistons per row.
  Redstone blocks carried in the rows: 0 rows (<= 2 blocks + 5 pistons). 11x6 fails by hand.
  satwire: r046x3-plane UNSAT.
* **7 wide**: row y=1 is blocked end to end (side pistons + their gaps + door), so each side's
  top and middle pistons are fed only from above and the bottom one only from below; 7x9/7x10
  give 3 regions whose links collide (flat-seam, box72). m2w (box72) wires the whole top of a 7x10
  and needs a 2 -> 1 converter in the bottom 3 rows.
* **cd19 (e89's family) in 8 rows**: column x=1 splits the box; e89's row -4 is its only
  crossing; a real observer watching B' double-fires into A's fatal window (flat-seam).
* **7x8 (56)**: box56's pulse-parity proof (0/99 catalog entries) is void: its even-link premise
  fails (observer busy window). 7x8-028/029 (v2 013/014) are blocked by one cell, K2's (4,4),
  which must serve the middle core piston (last gap 8|10 gt) and the bottom-right input (5..7 gt)
  (wire56). 038/039 and 001/002 have a piston fed only through another piston's cell, which can
  never fire alone (gadget `qc-shared`), or no feed at all; 000/003/004 need 45-46-event close
  trains (wire56). Sweep of the rest with timing variants: sweep56.
* **8x8 (64)**: 8x8-102 (v2 130): its bottom regions are reachable only through E3 (two pulses 7+
  gt apart) and R1 (4 gt apart), with no room for a converter (wire64, box64). v2 8x8-118..129:
  the elevator sits above BL's puller, so quasi-connectivity re-fires it (families). 139..142
  blocked by their own close paths (wire64).
* **7x8 with ML leaving vertically** (vmid, mechanisms only, ideal plans at 32 positions): vm1 (a
  column-0 elevator beside the center's, 11 pistons, missed by families' `--vmid` run at P <= 10),
  vc1 (top carried puller, C down: cannot be driven in-plane), vb2 / vb3 (bottom carried puller,
  mechanics.md section 8). `door connect`: vm1 NOT wirable (the twin elevators split the top: the
  lever's region opens but cannot close); vb1 / vb2: BM's puller has no feed or acts too late; vb3
  wakes every region but its order search caps (12 pistons), and by hand BL's puller (fed only from
  a 2-cell region under the carrier) must act before the row push that region watches. Freeing row
  y=1 does not by itself make 7x8 wirable: the center's elevator splits its side of the box, the
  bottom pullers split the bottom, and the carrier at (-2,-1) cuts the free left column.
* **w63's top (7x8-014 in 7x9 rows -2..6) by hand** (w63top NOTES "The wall, stated once"): A's
  only driver is an observer at (2,4) facing east; P_T's only real driver is an observer at (3,5)
  facing up; with it, every input of the right column (4,4)-(4,6), which must carry A's late
  pulses and the right stack's run, is P_T-coupled, and P_T may pulse only before A1 (with
  obsidian at (-1,3)) or after A3. Plans that do work ideally: A2 as a triple sharing (4,4) with
  the right stack (t1), P_T + right stack on one conductor as a run of 3 (v2), the lamp plan.
* **7 wide, 2 bottom rows** (w63top NOTES "Single-U 7x8 mechanisms"): the bottom rows can only
  watch the bottom side pullers, so one side must be a TOP-fed 2-stack (its top block leaves
  vertically, or BR leaves down so (4,0) watches row 1 as in c79). With 3-high stacks on both sides
  (the 9 catalog single-U mechanisms, mechspace's c78) the bottom never learns the lever.
  Mechanisms that pass this and work ideally at 32 positions with frame 12/12: u7-m8 / m9 / m10
  (m10 = c79 with BR right and TR up; m10a: its levels real).
* **No mechanism** (families catalog, <= 11 pistons, in-plane, lifts only, QC model): 6x11 and 11x6
  (so 6x9, 9x6, 6x10, 10x6), 5x14 and 14x5 in 2D (strips only via walking row mechanisms). 7x7:
  none found, 1,639 candidates unresolved. Carrying, helpers and 0-ticks are being added.
* **SUPER seamless** (covered reading) is impossible for solid doors: emptying the center needs a
  head or sticky face in the tunnel (seamless).
* **cosearch's "N of M inputs wired at 77/79" partials are broken doors** (cosearch2): with
  `--assist-weight 4` the annealer traded a door cell (1 point per cycle) for an assist release; in
  w63-2as the center never opens and the elevator is not wired (`door cosearch trace --assist
  --grid`). Only full-points partials count (`partial-full-live.txt`, weight < 1).
* **f131F-b2 / f131N-ff (wire131's 63b, one input left: N's train at the trapdoor (-2,4))**:
  N has ONE free neighbour, (-2,5); all 47 states of that cell screened with everything else real
  (`door cosearch pockets --keep-lever`): none gives the train in its windows (cosearch2 NOTES).
* **ring56 s1 (frame-12/12 7x8) is trigger-isolated at the top** (cosearch2): everything the
  top-left pocket can watch (ML puller, TL puller, C0) is fed only from top-left cells, everything
  the top-right pocket can watch (TM puller, TR puller, R2) only from top-right cells, and C1/C2
  carry no power: both need the lever, which cannot touch both. A mechanism needs a piston both top
  pockets touch (f131X's DE). Its bottom train is drivable piecewise: B' (both pulses, both edges)
  by 5 blocks watching BL's puller (`runs/pk-s1-k5/pocket0-set1.txt`).
* **One lever holds the whole ceiling (e88, 8x8 from e89b)**: a lever ON at rest standing on TOP of
  the T puller (1,4) (face=floor at (1,5), outside the volume) powers it directly and the TL/TR
  pullers (0,4)/(2,4) by QC (the lever is a neighbour of (0,5)/(2,5); its `updateNeighbours` reaches
  them); the extended puller's top face stays full, so the lever survives. The whole top row goes up
  with zero wiring and the TL/TR park watchers give one clean pulse each at t3 to both sides
  (`tracks/e88/wip/t3.txt`, `ideal/t3.ideal`; t3k: everything wired but one piston).
* **A lamp as merger input + delay (e88 t3n)**: the BR-puller watcher's two pulses (t9, t13) light a
  lamp (4,-2) that is also the input of a merger repeater (one A pulse at t15); observers on the lamp
  fire on both edges: the on-edge copy reaches B' while A' is busy (harmless), the off-edge copy
  (dark at t19) is B's boost at t25. Bottom right of an 8x8 fully wired with 8 blocks.
* **Left of a redstone-block shuttle (e88 t3k, 8x8)**: P_M (single pulse, t18..23) and the shuttle
  piston (push >= 25, pull >= push+5: A' sits at (1,-1) until the gate's pulse 1 ends, placed only
  at push+6) cannot both be driven from the 6-7 cells left of the shuttle plus an observer column:
  every P_M driver and every shuttle driver share one source, natural doubles are 4 gt apart
  (watchers of 2 gt pull-type pistons; the 2nd copy absorbed by the shuttle's own retraction still
  pulls one phase early), and any trigger off the redstone block (gate, lamp lit by it) re-fires when
  it returns: a clock. Pockets exhaustive to k=6 (78M fillings) and k=5 on 7 cells: no joint
  filling (`tracks/e88/NOTES.md`).

### Below 56: why 7x8 holds (2026-10-19 to 2026-10-21)

The record on the Solid and slime-free boards, and on vjeux's target, is **56 = 7x8x1**
([e78k-h5-41b](../designs/piston-door-3x3/archive/e78k-h5-41b.txt), 41 blocks). vjeux's target is
flat, slime-free, a FULL frame, nothing in front or behind, and the lever does not count. The
sculk board is also at 56 (c77's e78st-56b-38-1s-fill-37b-1, 37 blocks). From 10-19 20:00Z the
door team tried every box under 56: c70b/smaller, w86, macrosat (clockinfra), boxproof, pockets3,
pext, d67, clk-obs, clk-rt, clk-item and fly; each track's NOTES has the details. Mechanisms that
work with ideal inputs exist in almost every box. **None of them could be wired.** This section
says why, what is proven, and what the new gadgets did and did not change.

**In short.**
- Under 56, mechanisms that work with ideal inputs exist at 42, 48, 49 and 54. None could be wired
  by any tool or gadget we have.
- e78k's 56 uses a top-to-bottom crossing (a lever-held top trio + 2-piston side stacks) and room
  for the centre's elevator. That needs width >= 7 and height >= 8, so 7x8 is the smallest box with
  both.
- Every smaller box lacks one. 6-wide boxes have a box-edge side: no crossing, and a blind corner
  that drives at most one action per edge. 7x7, 7x6, 8x6 and 9x6 have no elevator room: the centre
  needs a parked piston or a 3-move path, and their pockets can't be fed. The one exception found
  is w86's **trio7** (7x7 = 49, slime-free, 10-21 06:58Z). It keeps the trio's crossing and sends C
  out through a middle frame cell; its only unwired piston is SU(1,-2) (4 close pulses from a
  sealed cell).
- Gadgets (carried and shuttled observers, slime, sculk sensors, timing-free stacks, outside
  levers) fixed starts and boxed pistons, but not loads: areas of 1-4 cells must make 3-10 distinct
  ticks per edge. Closest: trio7 (one piston), and q69c-sh0p (slime; every piston fed; one blind
  cell on one edge).

**What the 56 door uses.** These are two features of e78k. They are not proven necessary, but
every attempt below either keeps both or replaces one with a gadget.
- **A top-to-bottom crossing.** The door's three rows plus the two side columns cut the free cells
  into a group above the door and a group below it (box56's quadrant argument). One lever starts
  one group; the other group must SEE something the first one moves. e78k crosses through its side
  stacks: its lever-held top trio (one lever on the middle puller holds all three: e88's trick)
  takes the top row out of the side columns, so each side stack is only 2 pistons. A source above
  feeds both (direct + QC): a powered stone on the left, an observer on the right. An observer
  below ((-2,-1), (4,-1)) watches the lower one. This needs a spare column on EACH side and a
  spare row above: width >= 7. In smaller boxes only gadgets have carried a crossing: a slime
  column (q69m's islands pass 6 wide), and sculk sensors in the window model (m69T bot6, q69h,
  q69c). See the gadget table.
- **Room for the centre's 2-cell move**: two spare rows on one side for an elevator (e78k's
  bottom). With the trio's row: height >= 5 + 1 + 2 = 8. The other way for C is through a middle
  frame cell (a piston parked there, the w1 class), which fits 7x7.

So with an elevator, 7x8 is the smallest box with both. Every smaller box misses one, or (7x7) has
room for each but
not for both at once (except trio7, which replaces the elevator with a middle-frame exit):

| box | volume | crossing (a spare column each side + a row above) | elevator (2 spare rows) | both | mechanism? | what stops it |
|---|---|---|---|---|---|---|
| 7x5 .. 10x5 and their turns 5x7 .. 5x10 | 35-50 | no | no | no | NO | strip model: no row mechanism at any width <= 10, and turned (stacked) rows only lose options to quasi-connectivity (c70b, exhaustive) |
| 11x5, 5x11 | 55 | no | no | no | YES (strip model, 5 pistons per row) | every row mechanism fires twice from the inner door column and once from the ring column beside it, and the corner cell would need both roles: not wirable with fixed parts (strip's Lemma; carried redstone blocks: 0 rows) |
| 5x5x2 (2 deep, not flat) | 50 | - | - | - | NO | by argument (fly NOTES "5x5x2"): no block can change plane inside the box, and the door plane alone can't empty the opening and fill all 12 frame cells |
| 6x6 | 36 | no | no | no | NO (bounded) | no mechanism with <= 2 two-move blocks (families, all decided); macrosat UNSAT at h8 |
| 7x6 | 42 | yes | no | no | YES: w86 a3s, c70b a3s-11 | the centre's BM leaves through the centre column (a 3-move path); a3s-11's W fires inside the opening; not analysed further |
| 6x7 | 42 | no | yes | no | YES: c70b mm01..05 (a3s turned + repaired) | 2-4 pistons fire from inside the door square, never 0 (structural); d67: SU(1,-2) and the carried piston have no feed inside 6x7 (proof by cells); a 7th column would fix it (an observer at (-2,2)) |
| 8x6 | 48 | yes | no | no | YES: w86 a3r, macrosat m86-a | the centre's BM leaves through the centre column (a 3-move path); m86-a not wirable (one cell would need 8 distinct ticks); macrosat with `--physics --observers` UNSAT at h8 in (-4..3, -2..3) and (-4..3, -1..4), re-confirmed with the island-start fix (see Proven) |
| 6x8 | 48 | no | yes | no | YES: fly's s69 (slime-free; its mechanism spans 6x8 and its wiring rows make it 6x9), q68g (slime) | as 6x9 |
| 7x7 | 49 | yes | yes | not with an elevator: with the trio in the ceiling, C must leave through a middle frame cell (w86); w86's trio7 does exactly that | YES: c77 w1, c70b mm01 / u0 / u2, macrosat m7-a, w86 trio7 | **trio7** (explore/w86 `tracks/w86/seven/trio7.*`): u2 with a lever-held top trio (one real lever on SD+(1,4)'s top), TR up, static extras, C out through L1 via Q. WORKS with its real lever in redsim 32x3 and the game 4x2, FULL frame. Both top islands start at the flip. satwire's only core is SU(1,-2)'s close pulses (c24/c36/c48/c60, plus o30) from the sealed cell (0,-2); `door shuttle` finds 0 placements, and no pulse of it can be dropped (`prune-plan --exhaustive`). The sealed cell's neighbours are SU(-1,-2), SU(1,-2), the static (0,-1) and the outside, so its only possible source is an observer watching SU(-1,-2): SU(1,-2) can pulse only 2 gt after SU(-1,-2)'s moves (o42, o60, c48). Every filling of (0,-2), (-2,-1), (-2,-2): 0 of 1,728 (fly). w86 is re-planning for that lockstep. It works once the plan is compacted (`ideal-rs --compact-dir`: open 37 / close 13). SU(-1,-2)'s close pulse then falls at c7, and an observer at (0,-2) facing west replaces SU(1,-2)'s close c7 and c10 pulses: fly's t7o (explore/fly `wip/trio7/t7o.*`) WORKS redsim 32x3 + game 4x2. SU(1,-2) is left with one pulse at the start of each edge (compacted o3, c0, windows [+0,+0]), which no early SU(-1,-2) pulse can give (0 of 169; clk-rt 0 of 186), nor any 1-2-block filling of the 20 free cells (0 of 6,961, clk-rt). satwire on t7o (w86): the core moves to close SE(-2,0) at the close flip, because the bottom-left is blind at the start of the close (on the original timing: close Q (-1,-1) at c30, the same wall). macrosat on trio7's layout (h16, lever + island start + movable observers +
`--physics`) is SAT and places the same (0,-2) observer on its own, but its bottom-left makes many
distinct pulses from 2 cells, so it's a macro seed, not a wiring (clockinfra). pockets3's timed,
co-fire-aware island check reproduces the bottom-left close wall, and on w86's variant fr22 (SU(1,-2)
fed) it finds a second wall: the bottom-right on open. With the island-start fix, macrosat solves trio7 plus a movable observer at (3,3) facing west with every rule on (island start, `--physics`, movable observers, the lever switching only at the flips, the bottom-left limited to one action per edge). The export **replays**: explore/clockinfra 76f1ed250 `tracks/macrosat/sat/trio7/lf16a1-L.ideal` WORKS in redsim 32x3 (open 156, close 154), extent 7x7. `door prune-plan` drops 62 of its pulses together (236 -> 112 events; fly's `wip/trio7/lf16a1-L-p.ideal`, redsim 32x3 + game 4x2). Its 15 wiring cells are still air: the pulses there need real components, island starts and per-cell counts (pockets3, clk-item, w86 on it). With wiring made realistic (`--wire-dir`: one fixed-output component per wiring cell, plus the edge rule), trio7 is UNSAT at h16: once the sealed (0,-2) must hold one component, no 16-step schedule exists (clockinfra; h20 still running). Earlier 7x7s: C must pass a middle frame cell, so a piston parks there with >= 2 escape moves (w86), and there is no trio, so no crossing either. At one action per piston per edge none of the 1.57M decided candidates of families' ring-escape space (2.1M) is a mechanism; the other 0.53M are undecided at the cap (c70b). The w1 class leaves P (1,-1) and SU(1,-2) unfed; clk-rt's mm01z feeds P from a carried observer, but B (1,-2) stays unfed (5 pulses, no shuttle placement). c70b's u0 / u2-7x7 have every receiver fed, but their free cells form 5 islands and the right-side ones never start; 88 variants with every receiver fed reach at best an island start of 3, leaving 4-6 pulses per edge into a 2-cell island (c70b, d67, pockets3). clk-item's u2-T: with the lever on the static S(2,3)'s back face (it arms nothing), a torch at (2,4) holds SD(1,4) while the lever is off and gives the top-right island a started signal, but no filling uses it yet (redsim 32x3, game 4x2). Every other input of u2 still drives 0 times over every filling of its region, sensors, observers, wool, torches and repeaters included (0 of 1,156 to 45M). The wall is start order (w86): the stacks cross only downward and the top has no crossing, so with any one lever either the bottom-left or the top-right can't start. macrosat on u2-T's fixed layout (lever spot free, wiring free in its empty cells, h16 per edge) confirms it: every firing can be fed (SAT without island start), but no lever spot, wiring or movable-observer placement starts every island before its first firing (UNSAT with island start; re-confirmed with the pulse-visibility fix at h16, without observers and with movable observers + `--physics`; clockinfra) |
| 9x6 | 54 | yes | no | no | YES: c70b m69a turned, w86 m69g, c70b m69g-12 | one spare row, so the bottom row's pieces sit in a pocket that only row-0 movers touch. On w86's m69g, W' (3,-1) is fed only from the 2-cell pocket (4,-1),(5,-1): every filling with sensors, observers, wool, dust and torches drives it 0 of 324 times, and shuttles reach it only with one more row (clk-item; satwire core W', w86). c70b's m69g-12 fails the same way: SU(3,-1)'s only feed is the 2-cell pocket (4,-1),(5,-1), which drives it 0 of 1,156 times (sensors, observers, wool, torches, repeaters), and a shuttle works only with one more row. 9x7 = 63 fixes both (clk-item) |
| 6x9 | 54 | no | yes | no | YES: c78l m69a, c70b m69T, fly's q69c/h/i/j/m, r69t, s69 | below |

**6-wide boxes** (6x9, 6x8, 6x7) have no spare column on one side. That side frame holds the 3
extras, so it has no stack, and the other side's stack is 3 pistons tall: a source reaches only 2
of them from either end, so it cannot carry a crossing. A top trio does not fit either: it leaves
4 blocks for the 3 bottom slots. Measured on m69T (c70b's 6x9):
- no lever spot starts every area (pockets3's island check, corrected 00:00Z);
- with its schedule the two groups must start within 2 ticks of each other (fly, `door
  shift-plan`);
- interleave finds trains where one group waits 6-11 ticks (bot6, top6, bot11p), but no free cell
  sees the crossing moves: clk-obs's exhaustive fillings, 0 of 10M bottom and 0 of 16.8M top;
- an observer, or even a stone, parked in the column end cell (1,-1) breaks every train;
- the stack itself is a wall: one lever holds two of R0/R1/R2 (on R1's back: R1 + R0 by QC; on
  R2's back: R2 + R1), and the third one's only in-box feed is the back of a corner piston (R0's
  (4,-1) is K's back, R2's (4,3) is C's), so any level for it also fires K or C (clk-item,
  redsim: a perfect flip-flop output at (4,-1) leaves K extended at rest). Sensors fix m69T's
  starts (bot6, pockets3's `--sculk-islands`), but not this, and not its loads. Even with every
  spare pulse dropped (clk-rt), the sealed top-left (-1,4)(-1,5)(0,5) must fire s0 and W at 5
  distinct ticks per edge and the bottom-left (-1,-3..-1) at 4, from 3 cells each. Exhaustive
  pockets with real sensors drive nothing in either (pockets3). No input can be replaced by an
  observer watching the mechanism (`door selffeed`), and satwire with sensors is UNSAT (core R0).

The **box-edge column** is a second 6-wide wall (fly). Its middle block needs an elevator whose
pistons can only be fed from the corner at the box edge or from the centre column's cells, which
the centre's own elevator holds. On s69 that corner drives at most ONE action per edge (pockets3,
exhaustive), while the column needs about five; no shuttle reaches it either (clk-rt: it has no
3 collinear free cells). On the close edge nothing outside the corner can power A, B or K before
A's window ends (c34): (1,-2) holds Qv@ until B1's push (>= c38), which waits for K. A lever in
the corner would hold its pistons for the whole edge where they need short pulses, and a BUD start
needs A armed at rest, which the rules forbid (clk-rt). So s69 needs a different bottom-right, and
every slime-free 6-wide layout we found meets the same corner (fly's case analysis of where the
edge column's middle block can go, NOTES "STRUCTURAL RESULT"; not an exhaustive search). The
argument assumes the box-edge frame wall stays put, which holds when no piston fires from inside
the door square. That is a wirability bar (c70b), not a rule: c70b's 6x7 mechanisms fire 2-4
pistons from inside.

**Load.** The small mechanisms need 20-30 pulses per edge, many pistons pulse 3-9 times per edge,
and the free areas are 1-4 cells. A wirable door needs about 1-2 actions per piston per edge
(pockets3). Copper bulbs and comparators in the palette do not help such areas (clk-item:
exhaustive on q69m's 3-4-cell areas). The pistons with NO in-plane feed at all, per `door feeds`
on fly's 18 sub-56 plans (clk-item): none in q69c / q69h; W (-1,-1) in q69i/j/m; Qv (1,-1) (o6,
o18) and A@(2,-1) (o60, c48) in s69; SW(0,4) o10 in vq78; SD(1,3) o18 and SU(2,-1) o66 / c66 in
r69; 5+ in q68*, r69a, r69t.

**What the gadgets did** (each verified in redsim, most in the game):

| gadget | verified on | what it changed |
|---|---|---|
| lever outside the box on a never-moved piston's back face | fly's s69 (L0 + L1), c70b's m69T (R1 + R0) | holds that piston directly and the one below by QC: two levels for free (e88's lever on top of a row holds three) |
| timing-free side stacks | m69T's R0..R2 (1681/1681, bot6 too), s69's L0/L1 (41/41) | a lever-held stack works with each level switching anywhere 0..40 ticks after its flip, so it needs no flip-time signal and can be fed from any area |
| carried observer | fly's s69o (control without it: BROKEN); d67's 7th-column observer for 6x7 | a pushed or pulled observer fires 2 gt after landing and powers its output cell: a feed for boxed pistons |
| shuttled observer / redstone block (clk-rt's `door shuttle`) | s69r: s69's boxed Qv fully fed (o6, o18, Qv@ o30) plus B1 o24, game 4x2; fly's q69c-sh0: q69c's boxed Qv fed, game 4x2 | feeds pistons with no free neighbour; s69's corner pistons A, B, K have no 3 collinear free cells, so no shuttle reaches them; mm01z's bottom-middle B (5 pulses) has 0 placements |
| riding observer (a frame cell may hold one at closed rest) | concept (fly); in macrosat `--observers` | rides a column push into the other group and fires sideways: the one link that beats the end-cell catch below; m69T's end cells are taken |
| moving extras (the 3 extras may move mid-cycle, rules) | fly | a link in principle, but a 1-wide column's arrival is seen only from the column's continuation, where the restoring piston must stand |
| QC through an empty cell | fly's q69m (B1 fired through the empty (1,-2)) | counted by every model |
| slime column and slime riders (Solid board only) | fly's q68g, q69i/m | q69m is the only sub-56 candidate whose islands pass (the slime links top and bottom), but every area fails its pulse shapes and W has no feed |
| sculk sensors (sculk board only, also at 56) | pockets3's `--sculk-islands`; clk-item's real sensors in redsim | in the window model a sensor IS the missing crossing: islands pass on m69T bot6, q69h and q69c. Real sensors are harder. A piston's move emits the same game event as the lever (10 / 9), every piston of a 6x9 box is within a calibrated sensor's range (16; a plain one hears 8, and the box diagonal is 9.4), and a sensor re-arms 20 gt (calibrated) or 40 gt (plain) after it wakes, then re-fires on the door's own moves, which last 57-83 gt here. A sensor HOLDING a piston that must not re-fire breaks (all three cheapest starts on q69h / q69c). What works: an observer reading the sensor (each phase change, echoes included, becomes one 2 gt pulse) into a piston that tolerates extra pulses, with wool as a shield against the worst echo lines (clk-item's q69c-A2: A and the boxed Qv). One direct drive also works: q69h's W, from a calibrated sensor at (-2,-2) behind a wool shield. Both redsim 32x3 and game 4x2. m69T bot6 fails on load (its sealed top-left and bottom-left drive nothing, exhaustive) |
| carried redstone blocks, BUD arming, movable observers | macrosat (clockinfra), pockets3's screens | in the overnight searches |

**Closest approaches** (2026-10-21 07:00Z):
- Slime-free, the target: **trio7** (w86, 7x7 = 49; see the table). Its one unwired piston is
  SU(1,-2), fed only from the sealed cell (0,-2). With t7o's observer, its close train comes from
  SU(-1,-2)'s moves, leaving one pulse at the start of each edge. Everything else in trio7 still
  has ideal inputs. Re-checked: each fix moves satwire's core (next: the bottom-left blind at the
  close start). Exhaustive pockets on trio7 (pockets3, k=4, with sensors, wool, observers and bulbs,
  SU(1,-2) pinned) find the top-left driving nothing (7 ticks from 4 cells): at best 1 input is real.
  Latest: macrosat's lf16a1 (trio7 + a movable observer at (3,3)), solved with every rule on,
  replays WORKS in redsim 32x3. Its 15 wiring cells are the open work.
- Solid board, slime allowed: **q69c-sh0p** (fly, explore/fly `wip/q69c-sh0p.ideal`). It is q69c
  with clk-rt's shuttle for its boxed Qv, pruned by `door prune-plan`. Every piston has a free-cell
  feed (pockets3), and the islands fail at ONE cell on ONE edge: the bottom-right (2,-3) must start
  the shuttle by o23, and nothing it touches moves before o24. An observer parked in (2,-3) watching
  B1 fires 2 ticks late. With the lever in that corner instead, three other areas never start. A
  shuttle for S's open pulses: 4 placements reach them geometrically, 0 work (pockets3).
- Slime-free (vjeux's target): **s69** (fly) plus clk-rt's s69r (Qv fed by a riding observer).
  Besides the bottom-right corner (structural) and P_T's flip-time pulses, B1's close c66 can't be
  given on top of s69r: every feed for it jams through Qv (clk-item).
- Sculk board: **q69c-A3** (clk-item, explore/tff `tracks/clk-item/wip/q69c-A3`). A and the boxed
  Qv are driven by real calibrated sensors, and P_T by a stone riding the slime column that an
  observer watches (solid-valid). WORKS redsim 32x3. After `door prune-plan`, the 8 inputs left
  all fail single-input screens over their regions. The top has 5 of them ((0,4,1), and the box-edge
  column's ceiling machinery (2,3,1), (2,4,1), (2,5,1), K') for 3 free cells, 0 of 5,832 fillings
  each: the box-edge wall again, at the top. The bottom-left's 3 (G, (1,-2,1), (1,-3,1)) get 0 of
  1.9M each on 5 cells. pockets3's exhaustive pockets with sensors (every filling up to 4 blocks per
  area) agree. The top-left drives its whole load ((1,4) o0/c48 + (0,4) o8/c40, 4 blocks) and the
  bottom-right all of A (2 blocks). The bottom-left (G's level + (1,-2)'s 4 pulses, 7 cells) and
  the 2-cell top-right box-edge corner ((3,3) + (2,3)) drive nothing. Solid q69c-sh0p: the top-left
  drives (1,4) or (0,4), never both, and the other three areas nothing. Its top-right corner can't be
  reached by a shuttle (radius 0-3: 0 placements work), nor by pushing the extras column up into it:
  that column's bottom cell is the shuttle's landing cell, and a pushed column would shove K' (3,3),
  the piston it should fire (clk-item). With sensors, calibrated sensors and wool in the palette,
  sh0p's top-left still drives its whole load, but the bottom-left drives only B1 o30 (not
  (1,-2)'s train or G's level). The 1-cell bottom-right (S's 3 pulses) and the 2-cell top-right
  drive nothing: at best 3 of 10 inputs are real (pockets3). With the bottom-left's load pruned to
  the most that can go together (`prune-plan --only --exhaustive`, wip/q69c-blp), that area still
  drives only B1 (o30 + c56), never (1,-2)'s pulses, G's level or Qv (pockets3).

**Proven** (each within the stated model): 5-tall boxes up to 10 wide and their turns (strip
model); 6x6 with <= 2 two-move blocks; for 7x7 at one action per piston per edge, only that none of
the 1.57M decided candidates works (0.53M of 2.1M are undecided at the cap); macrosat's UNSATs,
which are bounded by the horizon (h6-h12) and its rules.
**macrosat's island-start bounds** (clockinfra). At 10-21 07:47Z clockinfra found a soundness bug in
island start: a pulse leaves its piston as it was, so the model never counted it as a change. A
wiring region next to a pulsing piston could not start from it, and a movable observer watching one
fired unseen. It is fixed in explore/clockinfra 36b146530. Every island-start UNSAT was re-run;
the lines below are the re-confirmed ones (fixed model):
- Plain feed + lever + island start, h6: UNSAT for 7x7 (-3..3, -3..3), (-3..3, -1..5),
  (-2..4, -3..3), (-3..3, -2..4) and (-2..4, -1..5).
- With the lever as a level, island starts, the `--physics` rules (co-firing, hold, redstone-block
  stability, QC arming) and movable observers (`--observers`; carried redstone blocks are in the
  palette from the start), no flat door is fed within 8 parallel steps per edge in 7x7
  (-3..3, -3..3) and (-3..3, -1..5), 6x8 (-1..4, -4..3) and (-1..4, -1..6), or 8x6 (-4..3, -2..3)
  and (-4..3, -1..4).
- u2-T's start order (above), h16; trio7 with one fixed-output component per wiring cell, h16.
Still running at the time of writing: the central 7x7 placement (-2..4, -2..4), the other 7x7 and
48 placements at h8 with `--physics --observers`, and s69 with its right column free (x 1..3). The
palette has no slime and no copper bulbs.
mm01-7x7 is a working mechanism that needs 20 steps even in the permissive model, so short-horizon
UNSATs do not rule 7x7 out. Closed with real blocks (clk-item, every filling of every region each
input can be fed from, after `door prune-plan`): q69h, where only W (-2,-1) is drivable (a
calibrated sensor at (-2,-2) plus a wool shield); every other input is 0 of 5.8k-1.9M. Also both
9x6 mechanisms, w86's m69g and c70b's m69g-12 (above). **Open**: long horizons; mechanisms outside
the generators; s69 with its corner re-designed (macrosat `--free` overnight).

New tools in this push: `door shift-plan` (re-time a plan, `@var` shifts for `ideal-rs --sweep`),
`door prune-plan` (drop the pulses a plan works without, together), `door shuttle` (clk-rt),
macrosat `--observers` / `--free` / `--region-budget` (clockinfra). pockets3's `cosearch pockets`
gained:
- a per-edge island start with causal attribution, where `--why-islands` says who starts each area
  and what each failing area sees;
- FEED rules for a lever outside the box on a never-moved piston's back;
- `--sculk-islands` (a necessary condition), with real-sensor screens via `--fill-palette
  ...,sculk,calibrated,wool`;
- `--pin-assist`, and a per-area LOAD line.
Also `door feeds` region cells (clk-item) and `door selffeed --anywhere`. Fixes: interleave honours
`--after` on its start, and `ideal-rs --compact-dir` keeps the plan's phase line.

## Tools built that night

All `mctas door <command>` ([tools.md](tools.md)): the judge and the boards (`verify`,
`leaderboard`, `frontier`), local search (`minimize`, `harvest`, `transplant`, `grow`, `regrow`),
exact synthesis (`synth`, `resynth`), mechanism synthesis (`mech-synth`, `mech-wire`), routers
(`route`, `route2`), integrity checks (`buildable`, `vanilla-check`, `vanilla-build`,
`vanilla-gadget`, `vanilla-fuzz`) and the showcase (`video-all`, `tick-video`, `gallery`,
`montage`, `world`).
