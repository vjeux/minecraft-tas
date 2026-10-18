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

Checked 2026-10-18 on main at 50b5616.

| board | design | blocks | buildable | lever while building | temporary blocks | what the plain order (mechanism, pistons, wiring, observers, power last) breaks | build script |
|---|---|---|---|---|---|---|---|
| solid #1 | `archive/fc2-40b.txt` | 40 | **yes**, 32/32 positions | off | 9 dirt supports, no helpers | a piston fires mid-build: the left slime column and door stones end one block west, the repeater at 0 6 0 pops off | [fc2](tracks/buildable/scripts/fc2.build.txt) |
| solid #5 | `archive/piston-door-3x3-fresh-b-mp8-43b.txt` | 43 | **yes**, 32/32 | off | 17 dirt supports | the bottom sticky piston and its slime are pushed down a block, two door stones missing | [mp8](tracks/buildable/scripts/piston-door-3x3-fresh-b-mp8.build.txt) |
| slime-free #1 | `archive/slime-free-s7q-44b.txt` | 44 | **yes**, 32/32 | off | 17 dirt supports | the door stones end shifted sideways, the repeater at 1 6 -1 pops off | [s7q](tracks/buildable/scripts/slime-free-s7q.build.txt) |
| slime-free #2 | `archive/slime-free-s7-45b.txt` | 45 | **yes**, 32/32 | off | 15 dirt supports | same as s7q | [s7](tracks/buildable/scripts/slime-free-s7.build.txt) |
| sculk #1 | `archive/st43-bmobs-42b.txt` | 42 | **yes**, 32/32 | off | 9 dirt supports | the repeater at 1 6 1 pops off (the found build places the sculk sensor last, so no later placement triggers it) | [st43](tracks/buildable/scripts/st43-bmobs.build.txt) |
| gravity #1 | `archive/gravity-g10-45b.txt` | 45 | **yes**, 32/32 | off | 12 dirt supports | pistons end in the wrong state, the repeater at 0 -6 -2 cannot be placed facing west at that point | [g10](tracks/buildable/scripts/gravity-g10.build.txt) |
| fastest solid (open 12 / close 7) | `archive/speed-wire-w6-109b.txt` | 109 | **yes**, 32/32 | off | 37 dirt supports | the lever and an observer cannot be placed in their orientation at that point, a piston head missing, 2 items dropped | [w6-109](tracks/buildable/scripts/speed-wire-w6.build.txt) |

Every leader is buildable, but none with the plain order: each needs a specific order. In the
plain order, pistons fire while the door is half placed and push parts of it out of place
(repeaters standing on moved blocks pop off), and some components can no longer be placed in
their orientation because every face that would give it is covered by then. The scripts give one
working order each. "dirt supports" are temporary blocks only needed because the judge's world is
a void (a player builds on the ground or against a pillar); they are simulated (their placement
and removal update the world like any block).

Self-test: `tracks/buildable/selftest-observer-pair.txt` (two observers facing each other, quiet
in the judge's rest state, a clock as soon as a player places the second): NOT FOUND, the build
never comes to rest.
