# The night of 2026-10-18: the smallest 3x3 piston door

At 05:14 UTC a coordinator agent started the first of 33 agent tracks on two dev servers. The
solid record was vjeux's v2-min at **115 blocks**; by 08:10 it was **40** (fresh-c's `fc2`).
Every design is judged by `mctas door verify --extra-offsets 28` from main (3 open/close cycles at
32 world positions). [LEADERBOARD.md](../designs/piston-door-3x3/LEADERBOARD.md) ranks 166 working
designs, and every design in the [archive](../designs/piston-door-3x3/archive/README.md) has a
[video](../designs/piston-door-3x3/archive/VIDEOS.md). Times are UTC commit times; this page
describes the state at 14:00.

**In one minute**

* **Solid: still 40, now three different doors.** fc2 (25 mechanism + 15 control blocks), riders'
  fx1 (fc2's core with its left column toggled by an odd number of pulses: 27 + 13) and wire21's
  m05 (a new family, mech-synth's 21-block pulse-only mechanism: 21 + 19). No 39 yet; the last
  section says what every search ruled out.
* **New board, asked by vjeux at 10:56: solid, flat (one z layer), no slime or honey.** 60 -> 49
  (flat track), 47 -> 44 with the mechanism turned upside down (flat-b), then **43** (lamp-sweep:
  a redstone lamp + dust replace 4 observers in flat's 49, after which a chain collapses into dust).
* **Slime-free 48 -> 41** (s7 45, s7q 44, wire19sf's 43 and 42 on mech-synth's 19-block mechanism,
  then lamp-sweep's 41), **gravity 45 -> 42** (`door resynth`), **sculk 42 -> 36** (sculk-2: a
  calibrated sculk sensor replaces a 5-block chain, then a moved pusher blocks an early pull),
  **fastest solid door: open 10 / close 7** (speed-2, 125 blocks; 11 / 7 now takes 118).
* **A stricter judge and two integrity checks.** Floating blocks (09:48) and randomly ticking
  blocks (12:03) are now rejected. Every board leader can be built by a survival player
  (`door buildable`), and the unmodified official server matches every leader tick for tick
  (`door vanilla-check`), including the build itself: `door vanilla-build` replays each build script
  with a bot player on the official server, identical after every click.

Every solid record, filmed by one camera (151 -> 40, 2:52):

[![every solid record](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.jpg)](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.mp4)

![the record of each board over the night](https://github.com/vjeux/minecraft-tas/releases/download/videos/night-timeline.png)

## The boards at 13:05

Ticks are game ticks after the lever flip, worst of 3 cycles
([board rules](piston-door-3x3.md#categories-and-boards); GIFs in
[GALLERY.md](../designs/piston-door-3x3/GALLERY.md)).

| board | best | blocks | open / close | watch | idea (track) |
|---|---|---|---|---|---|
| **Solid** | [fc2-40b](../designs/piston-door-3x3/archive/fc2-40b.txt) | **40** | 24 / 9 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.gif), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.mp4) | symmetric control, no hold (fresh-c) |
| Solid, tie | [riders-fx1-40b](../designs/piston-door-3x3/archive/riders-fx1-40b.txt) | 40 | 24 / 26 | [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/riders-fx1-40b.mp4) | fc2 with the left piston next to X's floor: X's 3 pulses per flip toggle it (riders) |
| Solid, tie | [m05-21b-sym-40b](../designs/piston-door-3x3/archive/m05-21b-sym-40b.txt) | 40 | 40 / 38 | [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/m05-21b-sym-40b.mp4) | a 21-block pulse-only mechanism; a lamp makes P_R's last pulse late (wire21, mech-synth) |
| Solid, no slime or honey | [lamp-sweep-w41-41b](../designs/piston-door-3x3/archive/lamp-sweep-w41-41b.txt) | 41 | 28 / 26 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/lamp-sweep-w41-41b.gif), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/lamp-sweep-w41-41b.mp4) | 19-block mechanism (every edge block on its own piston) on one pulse train (wire19sf's w42), re-synthesized: 2 stones + a cross dust replace a lamp, its floor and 2 observers (lamp-sweep) |
| **Solid, flat, no slime or honey** (new) | [lamp-sweep-flat43-43b](../designs/piston-door-3x3/archive/lamp-sweep-flat43-43b.txt) | 43 | 46 / 44 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/lamp-sweep-flat43-43b.gif), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/lamp-sweep-flat43-43b.mp4) | the 19-block mechanism in one plane (flat's 49), re-synthesized with a redstone lamp (lamp-sweep); flat-b's upside-down version is 44 ([flat-b-v7-44b](../designs/piston-door-3x3/archive/flat-b-v7-44b.txt)) |
| Sculk | [sculk2-s4-36b](../designs/piston-door-3x3/archive/sculk2-s4-36b.txt) | 36 | 20 / 37 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk2-s4b-36b.gif) (its twin s4b), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk2-s4-36b.mp4) | st43 (rb family) with its lever-to-R_B chain replaced by one calibrated sculk sensor, and the left pusher moved so its retracting base blocks an early pull (sculk-2) |
| Gravity | [synth-g42-42b](../designs/piston-door-3x3/archive/synth-g42-42b.txt) | 42 | 5 / 17 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/synth-g42-42b.gif), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/synth-g42-42b.mp4) | g10's sand drop tower, 3 sub-circuits re-synthesized (synth) |

| speed board | best | ticks | blocks |
|---|---|---|---|
| Fastest open | synth-g42-42b | 5 (close 17) | 42 |
| Fastest open, solid | [speed-2-a1-125b](../designs/piston-door-3x3/archive/speed-2-a1-125b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/speed-2-a1-125b.mp4)): the speed track's o7 without Q's hold and with an odd-tick pulse (speed-2) | 10 (close 7) | 125 |
| Fastest close | [speed-wire-s4-108b](../designs/piston-door-3x3/archive/speed-wire-s4-108b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/speed-wire-s4-108b.mp4)) | 7 (open 12) | 108 |
| Fastest open + close | [gravity-g8-48b](../designs/piston-door-3x3/archive/gravity-g8-48b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-g8-48b.mp4)) | 5 + 11 | 48 |
| Fastest open + close, solid | speed-2-a1-125b, also slime-free | 10 + 7 | 125 |

Other progression videos (re-cut at 13:20):
[sculk](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk-progression.mp4),
[slime-free](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-progression.mp4),
[gravity](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-progression.mp4) and
[flat](https://github.com/vjeux/minecraft-tas/releases/download/videos/flat-progression.mp4).

## How 115 became 40

| time | blocks | design | track | key idea |
|---|---|---|---|---|
| start | 115 | [v2-115b](../designs/piston-door-3x3/archive/piston-door-3x3-v2-115b.txt) | vjeux's session | v2 "UP dance"; 94 of its 115 blocks are wiring |
| 05:35 | 109 | [search-tool-v2-min-109b](../designs/piston-door-3x3/archive/search-tool-v2-min-109b.txt) | search-tool | local search: a reroute, then the lever net re-synthesized |
| 05:48 | **96** | [v2u2-96b](../designs/piston-door-3x3/archive/piston-door-3x3-v2u2-96b.txt) | center-mech | **sides pushed out** by pistons that are off at rest (no NOT lines) |
| 05:48-50 | 91, 87 | [mechanics-ta-91b](../designs/piston-door-3x3/archive/mechanics-ta-91b.txt), [v3c-87b](../designs/piston-door-3x3/archive/piston-door-3x3-v3c-87b.txt) | mechanics, top-control | A's torch next to its observer; side pistons high |
| 05:51 | **73** | [rb1-73b](../designs/piston-door-3x3/archive/piston-door-3x3-rb1-73b.txt) | redstone-block | **movable power**: one shuttled redstone block powers A, then the right piston, whose structure drags T aside (no P_T) |
| 05:53 | **57** | [rb2-57b](../designs/piston-door-3x3/archive/piston-door-3x3-rb2-57b.txt) | redstone-block | rb1 with short nets |
| 06:00-10 | 53, 51, 50, 48 | [mechanics-rb5-hold-48b](../designs/piston-door-3x3/archive/mechanics-rb5-hold-48b.txt), [rb6-48b](../designs/piston-door-3x3/archive/piston-door-3x3-rb6-48b.txt) | mechanics, redstone-block | Q's hold shrunk to a 7-block BUD "silent hold" |
| 06:44 | **47** | [rb6-bmobs-47b](../designs/piston-door-3x3/archive/rb6-bmobs-47b.txt) | top-control | **observer-fired bottom-middle** piston, retracted at rest |
| 08:08 | 46 | [mp6-46b](../designs/piston-door-3x3/archive/piston-door-3x3-fresh-b-mp6-46b.txt) | fresh-b | new family: **mirrored pump** |
| 08:09 | 42 | [fc1-42b](../designs/piston-door-3x3/archive/fc1-42b.txt) | fresh-c | new family: **symmetric control**, no hold |
| 08:10 | **40** | [fc2-40b](../designs/piston-door-3x3/archive/fc2-40b.txt) | fresh-c | fc1 with a pull-type pulse piston for the left side |
| 10:03 | 40, tie | [riders-fx1-40b](../designs/piston-door-3x3/archive/riders-fx1-40b.txt) | riders | fc2's left piston toggled by X's 3 pulses per flip; its 2 observers become 2 slimes |
| 11:25 | 40, tie | [m05-21b-sym-40b](../designs/piston-door-3x3/archive/m05-21b-sym-40b.txt) | wire21 | new family: mech-synth's 21-block pulse-only mechanism |

* **05:14-06:10, the wiring collapses (115 -> 48).** Search cleaned up v2's wires, then pushed
  sides and movable power deleted whole subsystems: the side NOT lines, P_T, A's nets.
* **06:10-08:08, the plateau.** Every search called 47-48 a local minimum, and Q's 7-block hold
  could not be made cheaper.
* **08:08-08:14, two new families.** The mirrored pump broke 47, symmetric control dropped the
  hold: 46, 42, 40.
* **08:14-13:05, no 39.** Instead, two different 40s that spend the blocks differently: fc2 has
  25 mechanism + 15 control, fx1 27 + 13, m05 21 + 19. (The sculk board, which allows sensors, is
  at 38 since 12:46.)

The other boards since 09:15:

| board | path | how |
|---|---|---|
| Slime-free | 48 -> 45 (09:18) -> 44 (09:36) -> 43 (11:56) | s7: fresh-c's close without slime (A held by a shuttled redstone block, P_T pulse-driven); s7q: `door regrow` made Q pulse-driven; w43: the 19-block mechanism on one pulse train |
| Flat (opened 10:56) | 60 (11:36) -> 57 (11:45) -> 55 (11:53) -> 54 (12:10) -> 48 (12:47) -> 47 (12:49) | flat ([flat-v2-54b](../designs/piston-door-3x3/archive/flat-v2-54b.txt), [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/flat-v2-54b.gif)): the 19-block mechanism in one plane, `minimize`, R_B fed by a dust under a powered block. flat-b: the same mechanism upside down, then the lever hung under the observer that watches it |
| Sculk | 42 -> 38 (12:46) | sculk-2: one calibrated sculk sensor (active 10 gt) replaces st43's 5-block lever-to-R_B chain |
| Gravity | 45 -> 42 (09:47) | `door resynth` rebuilt 3 of g10's sub-circuits smaller |
| Fastest solid | w6 12 / 7 at 161 blocks -> s4 12 / 7 at 109 (09:38) -> o2 11 / 7 at 137 (09:54) -> o7 11 / 7 at 119 (10:23); s4 minimized to 108 | a one-repeater silent hold; open 11 from a 0-tick inside one block-event phase |

## The door families

| family | best | idea |
|---|---|---|
| rb, movable power | rb6-bmobs 47; riders-r3 43 | pushed sides; a shuttled redstone block turns A off, then P_R on; the right structure drags T. Costs a hold for Q through the closing dance |
| Mirrored pump (fresh-b) | mp8 43, opens in 16 | a pump under the door; every piston fires once per edge, triggered by **flags** (blocks glued to moving structures, watched by observers) |
| Symmetric control (fresh-c) | fc2 40, fx1 40 | the same signals at the same delays on both edges. On close, A spits C down, T comes back, A pushes both home, and the B window pushes uselessly against the extended A ([walk-through](piston-door-3x3.md#the-current-best-fc2-40-blocks)) |
| Pulse-only 21 (mech-synth, wire21) | m05 40 | every piston retracted at rest and fired by 2 gt pulses, one 9-pulse train on both edges (P_L, A, P_T, B on, A, B off, A, M, P_R): no shuttle, no hold, no torch. T leaves north above the walkway |
| Slime-free 19 (mech-synth, wire19sf, flat, flat-b) | w43 43, flat-b 47 | every edge block on its own pull-type sticky piston, plus the A/B elevator and a T piston: 19 mechanism blocks, the floor of this family. One piston must fire after A's last pull: M at the bottom (t >= 22 on close in w43), or, with the elevator below the door (flat-b), the top pullers, which one powered block fires together |
| Slime-free shuttle (slime-free, fresh-d) | s7q 44, fresh-d s4 45 | fc2's close without slime: A held by a shuttled redstone block, side columns on pulse pistons |
| Gravity drop tower (gravity, synth) | g42 42, opens in 5 | sand sides pushed open by the doorway-floor pistons; the sand middle column drops 3 cells into a shaft on a collapsing 3-piston tower, which lifts it back |
| Sculk (top-control, search-tool, sculk-2) | sculk2-s1 38 | the rb family with a sculk sensor as Q's hold; a calibrated sensor (active 10 gt) also replaces the B trigger. Own board: footsteps trigger sensors |
| Speed (speed) | o7 119, 11 / 7 | T leaves first, a pusher shoves a sticky puller into T's exit cell, two quick pulls take C out; F waits armed (a BUD) to close in 7 |

v2's "UP dance", which started the night, is described in
[piston-door-3x3.md](piston-door-3x3.md#how-the-mechanism-evolved); the earlier slime-free designs
in their tracks' NOTES.md.

## Mechanics discoveries

Read in the 26.3 source, each proven by a [gadget](../designs/gadgets/) (a design plus a drive
script with `expect`s) or a judged door; details in [mechanics.md](mechanics.md).

* **Pulse-driven pistons** ([pulse-toggle](../designs/gadgets/pulse-toggle.drive),
  [spit](../designs/gadgets/spit.drive)). A retracted sticky piston one cell from its block, given
  one 2 gt pulse per edge, is a toggle: on open it reaches across the gap and pulls, on close it
  pushes and lets go. **Three pulses toggle it too** (fx1, m05): one net move, timed by the last
  pulse, so a piston can reuse a source that fires three times per flip (fc2's X).
* **Asymmetry for free.** A lamp lights at once and goes dark 4 gt after its power ends (m05's late
  P_R pulse). A waxed copper bulb toggles on rising edges only, so an observer on it pulses once
  per edge ([bulb-pulse](../designs/gadgets/bulb-pulse.drive)). A push-type and a pull-type pulse
  piston fed the same pulse act in opposite orders on the two edges
  ([order-flip](../designs/gadgets/order-flip.drive)).
* **Quasi-connectivity.** A piston is quasi-powered through the head of the piston above it, so
  fc2's X dust needs no conducting floor ([head-qc](../designs/gadgets/head-qc.drive); fc2-40b-2
  stands it on glass). A quasi-powered piston waits for an update
  ([silent-hold](../designs/gadgets/silent-hold.drive)), but a piston head forwards neighbor
  updates to its base, which is why fc2's Q cannot wait silently next to A (q-free).
* **Observer chains** cost 1 block per 2 gt. The tick at which a chain can fire a given cell is
  fixed mod 4 (aim at the piston's quasi cell to shift it by 2), and an observer watching a
  pulse-pulled door block fires 2 gt earlier on close than on open (wire19sf). A dust under a
  strongly powered block lights ([dust-below](../designs/gadgets/dust-below.drive), flat 54).
* **In one plane** (flat): repeaters cannot be locked (their sides face out of the plane), and a
  lever powers its neighbors, so it must stay off every piston and quasi cell.
* **Speed.** A piston 0-ticks when the redstone block powering it is shoved away in the same
  block-event phase (o2: open 11). Open 12 is the bound with 2 gt pulses, 7 / 6 with 0-ticks.
* **Sculk** (sculk-2, from the source). A vibration reaches a sensor max(1, floor(distance)) ticks
  after the event; a sensor stays active 30 gt (a calibrated one 10 gt), then deaf for 10. A lever
  flip and a piston's move are vibrations (a blocked extension is not); observers, repeaters and
  dust make none. A player standing on a sensor triggers it (vanilla-check).

Older findings (slime never drags glazed terracotta, the 12-block push limit, sand timing, pumps)
are in [mechanics.md](mechanics.md) and the gadget library.

## Tools built

All `mctas door <command>`; reference list in the [README](../README.md#tools).

* `verify`, the judge, and `leaderboard`, which verifies every design on main and `explore/*` with
  main's verifier and writes the boards.
* Local search: `minimize` (removals, moves, swaps, reroutes, net re-synthesis, plateaus), `harvest`
  (minimize on every branch's best), `transplant` (one lineage's edit tried on the others), `grow` /
  `regrow` (add the cheapest 1-2 blocks / rebuild every 2-3 block group with one fewer).
* Exact synthesis: `synth` (every circuit of up to k blocks between two cells) and `resynth`
  (clears every small group of wiring blocks and rebuilds it smaller: gravity 45 -> 42,
  slime-free 48 -> 46).
* `mech-synth`: mechanism synthesis in a macro piston model (a port of the game's piston structure
  resolver): pistons and slime around the door, open + close paths, symmetric pulse schedules,
  replayed with ideal inputs. Its 21- and 19-block mechanisms became m05, w43 and both flat doors.
* `route2`: a timing-aware net router (a wave model of dust, repeaters, observers, torches and
  lamps, matched tick for tick against the simulator on 8 records) that rips up and re-routes up to
  8 control blocks. It found no saving on any record.
* `buildable` and `vanilla-check` / `vanilla-fuzz`: see Correctness. Also `ideal`, `drive`, `trace`,
  `patch`, `route`, `place`, `reuse-check`, and `video-all` / `gallery` / `montage`.
* On branches, not merged yet: `obs-path` (shortest observer chains, wire19sf), `grow --then-search
  --glue` (rider seeds plus local search, riders), `synth --palette bulb` (wire21), `--to-sculk`
  (lets the searches turn a solid design into a sculk one, sculk-2).

## Correctness

* **The judge got stricter twice.** 09:48: blocks that could not stand where they are
  (`canSurvive`: floating levers, torches, dust, repeaters) are rejected; the minimizer had produced
  a floating lever, and no archived design was affected. 12:03: randomly ticking blocks are
  rejected: the buildable track found that fresh-d's unwaxed copper bulb oxidizes and its observer
  then fires the door by itself (`verify --cycles 40`: broken at cycle 20). Its unwaxed files are
  marked invalid; the waxed 45 stays. Rules: [piston-door-3x3.md](piston-door-3x3.md#the-rules-what-mctas-door-verify-checks).
* **Buildable in survival** ([BUILDABILITY.md](../designs/piston-door-3x3/BUILDABILITY.md)).
  `door buildable` searches for an order of real player placements (clicks on visible faces, the
  world updating after each one) that ends exactly in the judge's rest state, then runs the judge's
  cycles from there. All 13 board leaders as of 11:56 (fc2, fx1, s7q, st43, g42, the speed doors,
  flat 60...) are buildable at 32 of 32 positions, none in the naive order: observers fire pistons
  into the half-built door, sand falls, faces get covered. At 13:16 the coordinator ran it on the
  newer leaders too: fx1, m05, w42, flat-b 47 and sculk 38 are all buildable at 32 of 32 positions
  (the sculk door with the lever on while building).
* **Same as the real game.** `door vanilla-check` runs the official 26.3 server jar unmodified; a
  Rust bot clicks the lever and the region files are read back every tick. At 09:30: the 16 bests
  (fc2: 4,832 ticks) identical at 32 positions x 3 cycles plus a real-time cycle, every archived
  design at 4 positions, every branch best at 8, plus 140 random contraptions (`vanilla-fuzz`). No
  simulator divergence; each apparent one was a harness bug. In real play a player standing on a
  sculk sensor triggers it, which breaks rb2s2-50b and rb2-sculk-50b. At 13:29 the newer leaders
  passed too, identical tick for tick at 32 positions x 3 cycles plus a real-time cycle: fx1
  (5,024 ticks), m05 (7,904), w42 (5,408), flat-b 47 (6,368), g42 (3,758), sculk 38 (9,536) and the
  fastest solid o7 (2,912).
* **Simulator bugs fixed earlier**: torch burnout memory leaked across sandbox reuse (everything was
  re-judged), and the judge did not wait for falling sand.

## Open problems: what each search proved

* **Below 40.** fc2 and its variants are local minima of every search: `minimize` with plateaus
  (255 neutral designs at depth 2), `regrow` including `--any` (24.3M size-3 candidates screened
  before it was stopped, none passed), `grow` after cutting 3, `synth`, `resynth`, `route2` and
  riders' rider seeds; m05 too (`regrow --size 3`: 2.4M; no P_R driver under 6 blocks). By
  analysis, each fc2 control pair is pinned (fresh-c, fresh-e), P_M's 2 blocks are minimal in fc2
  and fx1 (riders), and a "free" Q delay fails on close because Q's head passes the A pulse's
  update to Q (q-free). mech-synth: with fc2-style slime sides and M fixed, no mechanism below 21
  blocks (19,967 combinations); without slime, 19 is the floor of its family. Leads: its catalogs
  (12 pulse-only 21s, 396 symmetric 22s, two without the B piston) ranked by wiring cost, odd pulse
  counts as a search move, one pulse source shared by P_L and P_M (each works at any tick 0..30).
* **Slime-free below 43.** w43 is a local minimum (`minimize --plateau`, `regrow --size 3` 2.1M,
  `synth` for M's trigger up to 6 blocks). The crux is M, which must move at t >= 22 on close (after
  A's last pull); a lamp only delays 4 gt, and the redstone block south of A costs 46 or more.
* **Flat below 47.** For the flat track's layout (the 19-block mechanism with the elevator on top)
  the late left chain is geometrically minimal (9 observers), and every flat slime-free pulse-only
  mechanism mech-synth found needs one late piston (66k combinations, 103 symmetric): the others
  need a piston riding a slime column. `regrow --size 3` (3.9M candidates), plateaus and `resynth`
  found nothing below 54. flat-b's 47 turns the mechanism upside down, so one late observer chain
  fires the left stack and then both top pullers; it has not been searched yet.
* **Gravity 42**: `resynth` and `route2` found nothing. **Sculk 38** is minutes old (sculk-2 is
  still on it).
* **Faster.** Open 10 works only with hand-placed inputs; the 0-tick bound is open 7 / close 6.
  The symmetric family's open floor is 22 (fc2f).

Still running at 13:05: wire21, wire19sf, flat, flat-b and q-free (box 1); mech-wire, sculk-2,
vanilla-build and speed-2 (box 2: open 10, or 11 / 7 with fewer blocks). Each track's
`designs/piston-door-3x3/tracks/<track>/NOTES.md` has what it tried and why things failed.
