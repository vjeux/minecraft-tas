# Buildability: can a survival player build the board leaders?

The judge pastes every block raw (no updates), sends dust shape updates and a self update to
every block, then settles. `mctas door buildable` (tool notes: `tracks/buildable/NOTES.md`)
searches for a build a survival player can follow: an order of real player placements (an item
click on a visible face of a neighbouring block, with the look direction that gives the
orientation; power, connections and piston extension are whatever the world makes of it), the
door lever off or on while building, temporary blocks placed and broken later. The world settles
after every action. A design is BUILDABLE when such a build ends exactly in the judge's rest state
(every block state of the judge's watch box, no dropped items) and the judge's 3 open/close cycles
pass from the built state, at all 32 judge positions.

Checked 2026-10-18 on main at f39c5c5. Build scripts: `tracks/buildable/scripts/<design>.build.txt`
(one line per action, design coordinates; `# note:` lines trace where the plain order breaks).

## Board leaders: all buildable

| board | design | blocks | buildable | dirt supports | where the plain order first goes wrong (mechanism, pistons, wiring, observers, power last; attached blocks after their support) |
|---|---|---|---|---|---|
| solid #1 | [fc2-40b-2](tracks/buildable/scripts/fc2-40b-2.build.txt) | 40 | **yes**, 32/32 | 9 | step 33/40: the observer on the left sticky piston must face up, and the only face giving that is a temporary block in the cell it watches; breaking it pulses the piston, which pulls the left slime column and the door's left column one block west |
| solid #2 | [fc2-40b](tracks/buildable/scripts/fc2-40b.build.txt) | 40 | **yes**, 32/32 | 9 | same as fc2-40b-2 |
| solid #4 | [riders-fx1-40b](tracks/buildable/scripts/riders-fx1-40b.build.txt) | 40 | **yes**, 32/32 | 9 | step 39/40: placing the lever is a change the observer next to it sees; the half-finished machine runs and pushes the door columns out of place |
| solid | [mp8-43b](tracks/buildable/scripts/piston-door-3x3-fresh-b-mp8-43b.build.txt) | 43 | **yes**, 32/32 | 6 | step 37/43: an observer placed against a temporary block in the cell it watches; breaking it fires the sticky piston behind the observer, and the piston column under the door's middle ends a block lower |
| slime-free #1 | [s7q-44b](tracks/buildable/scripts/slime-free-s7q-44b.build.txt) | 44 | **yes**, 32/32 | 10 | step 38/44: the observer placed on top of the observer that watches that cell makes it pulse; the three left sticky pistons fire with the door half built |
| slime-free #2 | [s7-45b](tracks/buildable/scripts/slime-free-s7-45b.build.txt) | 45 | **yes**, 32/32 | 11 | same as s7q |
| sculk #1 | [st43-42b](tracks/buildable/scripts/st43-bmobs-42b.build.txt) | 42 | **yes**, 32/32 | 5 | step 39/42: an observer placed against a temporary block in the cell it watches pulses when it is broken; the half-built machine runs. The working build places the sculk sensor last, so no later placement triggers it |
| gravity #1 | [synth-g42-42b](tracks/buildable/scripts/synth-g42-42b.build.txt) | 42 | **yes**, 32/32 | 11 | step 14/42: a sand door block placed before the block under it falls |
| gravity #2 | [g10-45b](tracks/buildable/scripts/gravity-g10-45b.build.txt) | 45 | **yes**, 32/32 | 7 | step 17/45: same (sand falls) |
| fastest open, open+close (solid) | [speed-wire-o6-119b](tracks/buildable/scripts/speed-wire-o6-119b.build.txt) | 119 | **yes**, 32/32 | 15 | step 115/119: the floor lever can no longer be placed facing north (every face that gives that orientation is covered by then) |
| fastest close (solid) | [speed-wire-s4-108b](tracks/buildable/scripts/speed-wire-s4-108b.build.txt) | 108 | **yes**, 32/32 | 13 | step 106/108: same (the lever) |
| fastest solid before o6/s4 | [speed-wire-w6-109b](tracks/buildable/scripts/speed-wire-w6-109b.build.txt) | 109 | **yes**, 32/32 | 12 | step 97/109: an observer can no longer be placed facing north |
| flat, slime-free #1 | [flat-v2-60b](tracks/buildable/scripts/flat-v2-60b.build.txt) (`origin/explore/flat:.../tracks/flat/BEST.txt`) | 60 | **yes**, 32/32 | 10 | step 30/55: the observer on the right side pistons must face up; its only face is a temporary block in the cell it watches; breaking it fires the right sticky pistons with the door half built |

Every leader is buildable with the lever off while building and no helper block, but only in the
right order: pistons must not fire while the door is half placed (observers see their neighbours
being placed, and temporary blocks being broken), sand needs its support first, and some
components have to go in while a face giving their orientation is still visible. "dirt supports"
are temporary blocks needed only because the judge's world is a void (a player builds on the
ground or against a pillar); they are simulated, their placement and removal update the world.

## Next rows

| design | blocks | buildable |
|---|---|---|
| [fc2-min-40b](tracks/buildable/scripts/fc2-min-40b.build.txt) | 40 | **yes**, 32/32 |
| [fc2f-41b](tracks/buildable/scripts/fc2f-41b.build.txt) | 41 | **yes**, 32/32 |
| [fc1-42b](tracks/buildable/scripts/fc1-42b.build.txt) | 42 | **yes**, 32/32 |
| [riders-r3-43b](tracks/buildable/scripts/riders-r3-43b.build.txt) | 43 | **yes**, 32/32 (the order found first places the lever ON and switches it off at the end, which runs one door cycle that settles the machine; `--lever off` finds a lever-off order too) |
| [sculk #2: search-tool-from-top-control-BEST-43b](tracks/buildable/scripts/search-tool-from-top-control-BEST-43b.build.txt) | 43 | **yes**, 32/32 |
| [gravity: transplant g6 45b](tracks/buildable/scripts/search-tool-transplant-HEAD-search-tool-from-HEAD-gravity-g6-49b-46b-45b.build.txt) | 45 | **yes**, 32/32 |
| [slime-free-combo-48b](tracks/buildable/scripts/slime-free-combo-48b.build.txt) | 48 | **yes**, 32/32 |
| [speed-wire-w1-114b](tracks/buildable/scripts/speed-wire-w1-114b.build.txt) | 114 | **yes**, 32/32 |
| [fresh-d-s4-45b](tracks/buildable/scripts/fresh-d-s4-45b.build.txt) | 45 | **31/32**: the copper bulb oxidized during the build at one position (below) |

## Survival hazard: the unwaxed copper bulb (fresh-d-s3-47b, fresh-d-s4-45b, fresh-d-s4-45b-2)

The copper bulb at 4 2 1 is watched by the observer under it. Random ticks oxidize unwaxed
copper (copper_bulb, exposed, weathered, oxidized): each stage is a block change that observer
sees, so the door fires on its own and ends broken. The judge's 3 cycles are usually too short to
see it; a longer run is not: `door verify fresh-d-s4-45b.txt --cycles 40 --extra-offsets 28` is
BROKEN (offset [9, 37, 8], cycle 20), and the build check above failed at one position with the
bulb turned exposed_copper_bulb. The same design with a waxed copper bulb (honeycomb; same 45
blocks), `tracks/buildable/fresh-d-s4-waxed.txt`, WORKS for 40 cycles at all 32 positions and is
buildable at 32/32 ([script](tracks/buildable/scripts/fresh-d-s4-waxed.build.txt)).

## Self-test

`tracks/buildable/selftest-observer-pair.txt` (two observers facing each other: quiet in the
judge's rest state, a clock as soon as a player places the second): NOT FOUND, the build never
comes to rest, blocking: observer.

## Replayed in the real game (vanilla-build track)

`mctas door vanilla-build <script>` replays these scripts in the UNMODIFIED 26.3 server with a bot
player (item in hand, the script's look direction, sneaking, a right-click on the given face;
temporary dirt placed and broken by the bot too), compares vanilla with the simulator after every
action, the built world with the judge's rest state, then runs the judge's cycles with the bot.
Every leader above, plus the newer leaders wire19sf-w43-43b, flat-v2-54b and sculk2-s1-38b
(scripts in `tracks/vanilla-build/scripts/`), PASSES: identical after every action, built world =
rest state, cycles identical tick for tick (4 positions; fc2-40b-2 at 32; also built at real-time
20 tps). Details, the method and the findings (a player standing by st43's sensor makes it fire one
extra time after step 26; the flat script belongs to the 55-block revision 07bdbd0, not
`flat-v2-60b.txt`; 12% of clicks have no standing spot on their exact view ray; the build op sets
repeater delays and the lever in the same tick as the placement) are in
[tracks/vanilla-build/NOTES.md](tracks/vanilla-build/NOTES.md).
