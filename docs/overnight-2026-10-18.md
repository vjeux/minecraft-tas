# The night of 2026-10-18: the smallest 3x3 piston door

At 05:14 UTC a coordinator agent started the first of about 20 agent tracks on two dev servers.
The solid record was vjeux's v2-min at **115 blocks**; by 08:10 it was **40** (fresh-c's `fc2`).
Every design was judged by `mctas door verify --extra-offsets 28` from main (3 open/close cycles
at 32 world positions), and the records match the unmodified vanilla server tick for tick.
[LEADERBOARD.md](../designs/piston-door-3x3/LEADERBOARD.md) ranks 123 working designs, and the
[archive](../designs/piston-door-3x3/archive/README.md) keeps 125 design files, each with a
[video](../designs/piston-door-3x3/archive/VIDEOS.md). Times are UTC commit times.

Every solid record, filmed by one camera (151 -> 40, 3:11):

[![every solid record](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.jpg)](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.mp4)

![the record of each board over the night](https://github.com/vjeux/minecraft-tas/releases/download/videos/night-timeline.png)

## The boards at the end of the night

Ticks are game ticks after the lever flip, worst of 3 cycles
([board rules](piston-door-3x3.md#categories-and-boards); GIFs in
[GALLERY.md](../designs/piston-door-3x3/GALLERY.md)).

| board | best | blocks | open / close | watch | idea (track) |
|---|---|---|---|---|---|
| **Solid** | [fc2-40b](../designs/piston-door-3x3/archive/fc2-40b.txt) | **40** | 24 / 9 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.gif), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.mp4) | symmetric control, no hold (fresh-c) |
| Solid, no slime or honey | [slime-free-combo-48b](../designs/piston-door-3x3/archive/slime-free-combo-48b.txt) | 48 | 24 / 50 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-combo-48b.gif), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-combo-48b.mp4) | a pulse-fired piston per side block (slime-free, on logic-min's 50) |
| Sculk | [st43-bmobs-42b](../designs/piston-door-3x3/archive/st43-bmobs-42b.txt) | 42 | 22 / 38 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/st43-bmobs-42b.gif), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/st43-bmobs-42b.mp4) | rb family, a sculk sensor as the hold (top-control, search-tool) |
| Gravity | [gravity-g10-45b](../designs/piston-door-3x3/archive/gravity-g10-45b.txt) | 45 | 5 / 19 | [GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-g10-45b.gif), [video](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-g10-45b.mp4) | sand column on a 3-piston drop tower (gravity) |

| speed board | best | ticks | blocks |
|---|---|---|---|
| Fastest open | gravity-g10-45b | 5 (close 19) | 45 |
| Fastest open, solid | [speed-wire-w1-114b](../designs/piston-door-3x3/archive/speed-wire-w1-114b.txt) | 12 (close 19) | 114 |
| Fastest close | fc2-40b | 9 (open 24) | 40 |
| Fastest open + close | [gravity-g8-48b](../designs/piston-door-3x3/archive/gravity-g8-48b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-g8-48b.mp4)) | 5 + 11 | 48 |
| Fastest open + close, solid | [speed-wire-w5-122b](../designs/piston-door-3x3/archive/speed-wire-w5-122b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/speed-wire-w5-122b.mp4)), also slime-free | 12 + 11 | 122 |

Among small solid doors, [mp8-43b](../designs/piston-door-3x3/archive/piston-door-3x3-fresh-b-mp8-43b.txt)
opens fastest (16 / 16,
[GIF](https://github.com/vjeux/minecraft-tas/releases/download/videos/piston-door-3x3-fresh-b-mp8-43b.gif)),
and [fc2f-41b](../designs/piston-door-3x3/archive/fc2f-41b.txt) (22 / 9) has the best total.
Record-progression videos:
[slime-free](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-progression.mp4),
[sculk](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk-progression.mp4),
[gravity](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-progression.mp4).

## How 115 became 40

| time | blocks | design | track | key idea |
|---|---|---|---|---|
| start | 115 | [v2-115b](../designs/piston-door-3x3/archive/piston-door-3x3-v2-115b.txt) | vjeux's session | v2 "UP dance"; 94 of its 115 blocks are wiring |
| 05:35 | 109 | [search-tool-v2-min-109b](../designs/piston-door-3x3/archive/search-tool-v2-min-109b.txt) | search-tool | local search: a reroute (113), then the lever net re-synthesized |
| 05:48 | **96** | [v2u2-96b](../designs/piston-door-3x3/archive/piston-door-3x3-v2u2-96b.txt) | center-mech | **sides pushed out** by two pushers under the walkway that share one obsidian block and are off at rest (no NOT line). Redstone blocks riding the side structures power A and the bottom piston |
| 05:48-50 | 91, 87 | [mechanics-ta-91b](../designs/piston-door-3x3/archive/mechanics-ta-91b.txt), [v3c-87b](../designs/piston-door-3x3/archive/piston-door-3x3-v3c-87b.txt) | mechanics, top-control | A's torch next to its observer; side pistons high, redstone blocks on the backbones |
| 05:51 | **73** | [rb1-73b](../designs/piston-door-3x3/archive/piston-door-3x3-rb1-73b.txt) | redstone-block | **movable power**: one redstone block, shuttled by one piston Q, powers A at rest, then the right piston. The right structure drags T aside, so P_T is gone |
| 05:53 | **57** | [rb2-57b](../designs/piston-door-3x3/archive/piston-door-3x3-rb2-57b.txt) | redstone-block | rb1 with short nets |
| 06:00-10 | 53, 51, 50, 48 | [mechanics-rb5-hold-48b](../designs/piston-door-3x3/archive/mechanics-rb5-hold-48b.txt), [rb6-48b](../designs/piston-door-3x3/archive/piston-door-3x3-rb6-48b.txt) | mechanics, redstone-block | Q's hold as a short U, a 4-block pulse bridge, then rb5's BUD "silent hold" in 7 blocks |
| 06:44 | **47** | [rb6-bmobs-47b](../designs/piston-door-3x3/archive/rb6-bmobs-47b.txt) | top-control | **observer-fired bottom-middle**: the bottom piston is retracted at rest, fired by an observer watching the bottom-right door block |
| 08:08 | 46 | [mp6-46b](../designs/piston-door-3x3/archive/piston-door-3x3-fresh-b-mp6-46b.txt) | fresh-b | new family: **mirrored pump** |
| 08:09 | 42 | [fc1-42b](../designs/piston-door-3x3/archive/fc1-42b.txt) | fresh-c | new family: **symmetric control**, no hold |
| 08:10 | **40** | [fc2-40b](../designs/piston-door-3x3/archive/fc2-40b.txt) | fresh-c | fc1 with a pull-type pulse piston for the left side |

* **05:14-06:10, the wiring collapses (115 -> 48).** Search cleaned up v2's wires. Then pushed
  sides and movable power deleted whole subsystems: the side NOT lines, P_T, and A's early and late
  nets.
* **06:10-08:08, the plateau.** Every search called 47-48 a local minimum, and mechanics showed
  Q's 7-block hold could not be made cheaper. Meanwhile the other boards filled up.
* **08:08-08:14, two new families.** In under two minutes the mirrored pump broke 47 and symmetric
  control dropped the hold: 46, 42, 40. Then came mp8 43, fc2f 41 and
  [riders-r3-43b](../designs/piston-door-3x3/archive/riders-r3-43b.txt) (22 / 33), an rb door whose
  hold is an observer riding the left structure.

## The door families

* **v2 "UP dance"** (before the night). Slime backbones pull the side columns, A lifts T and P_T
  pulls it aside. C climbs on A riding B: B pushes A down, A pulls C up, B lifts A, A pulls again.
  Pulse-driven rewrite: [logic-min-v2-49b](../designs/piston-door-3x3/archive/logic-min-v2-49b.txt).
* **rb, movable power** (73 -> 47; riders 43). Pushed sides, one shuttled redstone block for "A off,
  then P_R on", and the right structure drags T. The cost is Q's hold through the closing dance.
* **Top pump** (fresh-b, [pd3d 53](../designs/piston-door-3x3/archive/piston-door-3x3-fresh-b-pd3d-53b.txt)).
  A pump pushes the top row and C down, then pulls back only the row. Its control has to climb
  above and below the door.
* **Mirrored pump** (fresh-b, mp6 46 -> mp8 43). A pulls TM up. A pump under the door lifts a
  2-slime row so the bottom-middle pushes C into TM's old cell. The right structure drags TM away
  and A pulls C out the top. Every piston fires once per edge, in the same order, triggered by
  **flags**: blocks glued to the moving structures, watched by observers.
* **Symmetric control** (fresh-c, fc1 42 -> fc2 40). The same signals on both edges. On close,
  A spits C down one cell, T comes back, and A pushes both home, while the B window pushes
  uselessly against the extended A.
  [Walk-through](piston-door-3x3.md#the-current-best-fc2-40-blocks).
* **Gravity drop tower** (g1 69 -> g10 45). Sand sides ride backbones pushed by the doorway-floor
  pistons. The sand middle column drops 3 cells into a shaft on a collapsing 3-piston tower, which
  lifts it back.
* **Slime-free pulse pistons** (observer-seq 89 -> 58, logic-min 50, slime-free 48). Each side
  block has its own retracted sticky piston, and one cross dust fires a stack of three.
* **Sculk** (50 -> 42). A sculk sensor is a one-block 30 gt hold, woken by each lever flip. Its own
  board, since footsteps trigger it; with no hold left in fc2, solid has passed it.
* **Speed** (w1 -> w5). T leaves first, a pusher shoves a sticky puller into T's exit cell, and two
  quick pulls take C out. The track's bound for this layout is 7 / 6.

## Mechanics discoveries

Read in the 26.3 source, each proven by a [gadget](../designs/gadgets/) (a design plus a drive
script with `expect`s); details in [mechanics.md](mechanics.md).

* **Pulse-driven pistons** ([pulse-toggle](../designs/gadgets/pulse-toggle.drive),
  [spit](../designs/gadgets/spit.drive)). A retracted sticky piston one cell from its block, given
  one 2 gt pulse per edge, is a toggle. On open it reaches across the gap and pulls the block (fc2's
  P_L pulls the whole left structure); on close it pushes the block back and leaves it. No torches
  or steady power.
* **Movable power.** Shuttling a redstone block from piston A to piston B turns A off at once and B
  on 2 gt later. A redstone block riding a structure switches pistons as the structure moves.
* **Blocked push** ([blocked-push](../designs/gadgets/blocked-push.drive)): pushing into an extended
  piston does nothing, so fc2's B window can fire on both edges.
* **Silent hold / BUD** ([silent-hold](../designs/gadgets/silent-hold.drive),
  [qc-bud](../designs/gadgets/qc-bud.drive), [head-forward](../designs/gadgets/head-forward.drive),
  [dust-reach](../designs/gadgets/dust-reach.drive)). A quasi-powered piston waits for a neighbor
  update, repeaters update quietly, and piston heads forward updates.
* **Observers as triggers** ([dual-edge](../designs/gadgets/dual-edge.drive),
  [observer-delay](../designs/gadgets/observer-delay.drive),
  [flag-observer](../designs/gadgets/flag-observer.drive)). Watching the lever gives one pulse per
  flip; chains cost 1 block per 2 gt with no floors; watching a piston gives two pulses.
* **Pump** ([pump](../designs/gadgets/pump.drive), [slime-rider](../designs/gadgets/slime-rider.drive)):
  push a 2-block line, pull back only the first block. A piston under a slime row rides it.
* **Sculk** ([sculk-hold](../designs/gadgets/sculk-hold.drive)): a 30 gt monostable on both edges,
  then deaf for 10 gt.
* **Slime** ([slime-nonstick](../designs/gadgets/slime-nonstick.drive),
  [honey-slime](../designs/gadgets/honey-slime.drive)). Slime never drags glazed terracotta,
  obsidian, extended pistons or dust, so floors beside backbones are glazed terracotta. Slime and
  honey ignore each other. A push moves at most 12 blocks.
* **Timing**: [pulse-stretch](../designs/gadgets/pulse-stretch.drive),
  [early-window](../designs/gadgets/early-window.drive),
  [repeater-lock](../designs/gadgets/repeater-lock.drive),
  [landing-fire](../designs/gadgets/landing-fire.drive) (a piston pushed onto a powered cell
  extends as it lands), [no-retry](../designs/gadgets/no-retry.drive) (a piston blocked by a
  moving block never retries), [stack-feed](../designs/gadgets/stack-feed.drive) (one dust fires 3
  pistons).
* **Sand and dust** ([sand-drop-tower](../designs/gadgets/sand-drop-tower.drive)). Sand falls 2 gt
  after landing unless supported (a moving piston counts). A lone dust is a dot; write it as a
  cross. Dust update order depends on the position, hence 32 test positions.

## Tools built

All `mctas door <command>`; reference list in the [README](../README.md#tools).

* `verify`: the judge (vjeux's session), extended with `--extra-offsets`, legality checks and categories.
* `leaderboard`: verifies every design on main and `explore/*` with main's verifier; writes the boards.
* `minimize` (search.rs): local search (removals, moves, swaps, reroutes, net re-synthesis), ~8,000 screens/s.
* `harvest`: runs `minimize` on every branch's best designs.
* `transplant` (explore/search-tool, not merged): applies one lineage's edit to another.
* `grow` / `regrow`: add the cheapest 1-2 blocks to a cut design / rebuild 2-3 blocks with one fewer.
* `ideal`: judges a mechanism driven by ideal scheduled inputs; sweeps its timing windows.
* `drive`: scripted inputs, probes and `expect` checks; the gadget test runner.
* `patch`: a design as a small edit script on its parent.
* `route`: dust auto-router, now aware of the cells a drive script sweeps.
* `place`: every combination of alternative part placements.
* `reuse-check`: one design verified many times in one sandbox (it found the torch leak).
* `video-all`, `gallery`, `montage`: a video per design, a GIF per board, one-camera progressions.
* `vanilla-check`: compares a design with the official server, tick for tick.
* `synth` (explore/synth, not merged): exhaustive circuit synthesis.

## Simulator bugs found and fixed

* **Torch burnout leaked across sandbox reuse** (logic-min, search-tool; bridge fix 4f45e1e).
  `Sandbox.clear()` rewinds the clock, but `RECENT_TOGGLES` only forgets a torch toggle 60 ticks
  later, so reused torches burnt out. logic-min's slime-free 50 passed 8 of 30 reused runs before,
  30 of 30 after. Searches had been rejecting good torch designs; everything was re-judged.
* **The judge did not wait for falling sand** (gravity, 393a327): `Sandbox.idle()` now does.
* Tooling: `canonicalize` let pistons fire during `gen` (top-control); videos had stale X locks and
  a 2-chunk view distance (showcase).

## Vanilla cross-check

`door vanilla-check` ([method](../designs/piston-door-3x3/tracks/vanilla-check/NOTES.md)) runs the
official 26.3 server jar unmodified. A Rust bot clicks the lever, and the region files are read
back every tick and compared cell by cell. **No divergence.** The 16 current bests (fc2: 4,832
ticks) matched at all 32 positions x 3 cycles plus a real-time 20 tps cycle. Every branch's BEST
matched at 8 positions, and older and archived designs at 4-8; a random-contraption fuzz is still
running. Each apparent divergence was a harness bug, such as `save-all` leaving stale tick lists.
The full results land with the track's next merge.

## Open problems and next ideas

* **Below 40.** fc2 is a local minimum for `minimize` (plateau pairs, 100-step walk), `regrow`
  (1.6M candidates), `grow` and `synth`. Its 15 control blocks: lever 2, Q's delay 2, early A pulse
  3, B window 4, P_L 2, P_M 2. fresh-c's lead is one pulse source shared by two of P_L, P_M and the
  window (39). It expects symmetric control on the mirrored pump to tie unless the bottom shares.
* **Slime-free below 48.** fresh-c's close works without slime with ideal inputs (-2 blocks), but
  needs an A source next to A that turns off fast and on slowly.
* **Faster.** The symmetric open floor is 22 (fc2f); a split B window might give 21 (ideal study
  only). Solid open + close is 23 against the speed track's bound of 7 + 6.

Still running at ~09:30: fresh-c, fresh-d, slime-free, speed, riders, synth, mech-synth and
vanilla-check. Each track's `designs/piston-door-3x3/tracks/<track>/NOTES.md` has what it tried
and why things failed.
