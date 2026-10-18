# vanilla-check track

Goal: prove the simulator's verdicts match REAL vanilla Minecraft 26.3, not design doors.

## The tool: `mctas door vanilla-check <design>...`

```
./target/release/mctas door vanilla-check designs/piston-door-3x3/archive/rb6-bmobs-47b.txt \
    [--extra-offsets 28] [--positions N | --offset=x,y,z] [--cycles 3] [--trace] [--keep-world]
./target/release/mctas door vanilla-fuzz [--count 50] [--seed 1]    # random contraptions
```

* Starts the OFFICIAL server jar unmodified (`java -jar server-bundle-26.3.jar nogui`, the jar
  `mctas setup` downloads) in a fresh world under /dev/shm: void superflat
  (`{"layers":[{"block":"minecraft:air","height":1}],"biome":"minecraft:the_void"}`), peaceful,
  `spawn_mobs`/`spawn_monsters`/`advance_time`/`advance_weather` off (the judge's sandbox
  rules), RCON on a free port, online-mode off, compression off, `/tick freeze`, `/save-off`.
* A tiny offline game client written in Rust (`crates/mctas/src/vanilla/bot.rs`, protocol 777)
  logs in as `mctas_bot` (creative) and RIGHT-CLICKS THE LEVER (`use_item_on`) from a spot within
  reach that touches no block of the build (so nothing pushes it and it stands on nothing: no
  footstep/landing vibrations), outside the watch box and far from sculk sensors when possible.
  Server side that is `LeverBlock.useWithoutItem` -> `pull(state, level, pos,
  null)`, the very call the judge makes (a `/setblock` of a powered lever would NOT be the same:
  it skips the attached block's neighbor updates and the vibration game event).
* World readback: `save-all flush` + a Rust region-file reader (`vanilla/mod.rs`, `nbt.rs`):
  every block state of the judge's watch box after every tick, plus the saved scheduled block
  ticks and block entities (moving pistons) for the judge's `idle` (falling blocks via
  `execute if entity @e[type=falling_block]`). 26.x quirks: dimensions live in
  `world/dimensions/minecraft/overworld/region`, palette entries are a bare name for a block's
  DEFAULT state (expanded through the simulator), `{id, properties}` otherwise, and strings in a
  mixed list are wrapped as `{"": name}`.
* Per position (the design translated by the judge's offsets; SAME absolute coordinates in both
  worlds, since dust update order is position dependent):
  1. simulator: the judge's exact sequence (raw set, wire shape pass, `self` update of every
     block, settle), recording the watch box every tick;
  2. vanilla: clean the box (`fill air strict`, kill entities, 80 ticks so pending ticks and the
     torch burnout memory expire), `forceload` + wait for `execute if loaded`, bot teleported
     near the lever BEFORE placement, then `/setblock <pos> <state> strict` of every block
     (strict = flags 2|816 = UPDATE_CLIENTS|KNOWN_SHAPE|SUPPRESS_DROPS|
     SKIP_BLOCK_ENTITY_SIDEEFFECTS|SKIP_ON_PLACE, bit for bit the judge's RAW_FLAGS), using the
     states after the judge's wire shape pass; then `tick step 1` + readback until idle, flip,
     step until idle, flip, ... 3 cycles;
  3. both runs go through the same rule checks (`vanilla_check::judge`, a copy of `verify_at`:
     closed at rest, lever port, open + walkway walkable, door restored, whole box back to rest,
     no entities) and every frame is compared cell by cell. Output: PASS (identical tick for
     tick), FAIL (vanilla fails the rules, frames identical to the simulator), or DIVERGES
     (first differing tick and cells).
* Idle and flip schedule: vanilla steps exactly as many ticks as the judge did in each phase
  (when the judge settled), so every lever flip happens at the same tick in both worlds; at that
  tick vanilla must be quiet on its OWN observable state (saved block/fluid ticks, moving
  pistons, falling blocks), else it keeps stepping and the extra frames are reported. Block
  events are not saved to disk; a hidden one would surface as a frame difference afterwards.
* Real-time cycle (first position, default on): `/tick unfreeze` at vanilla's 20 tps, the bot
  clicks while the game runs, the world is frozen and read back only after the judge's settle
  time + 20 ticks; the open and closed states must equal the simulator's.
* Known gap: vanilla has no command for the judge's `self` update (neighborChanged on every
  block after placement). Canonical designs are already settled, so it is a no-op for them. A
  file that is NOT its own rest state is now checked from its simulator rest state (printed as
  a note); without that it shows up as a DIVERGES at `settle` (see `selftest-bud.txt`: a
  quasi-powered retracted piston extends in the judge, stays retracted in vanilla; the tool
  reports `settle tick 1: [7, 0, 0] sim sticky_piston[extended=true] / vanilla
  sticky_piston[extended=false]; [7, 1, 0] sim moving_piston / vanilla air`).

## Harness pitfalls found on the way (fixed; each one first looked like a game divergence)

* A PLAYER STANDING ON A SCULK SENSOR TRIGGERS IT (real vanilla behavior, not a simulator
  bug): the first bot spot was the free cell closest to the lever, which for rb2s2-50b and
  rb2-sculk-50b is right on top of the sculk sensor. At some flips the sensor then fired with
  power 15 (a vibration source < 0.53 blocks away: the bot's feet) one tick before the lever's
  own vibration (power 11, 2 ticks of travel) would wake it; stepped closes took 77 ticks
  instead of 37 and in the real-time cycle the right column ended one block off (door broken).
  Repro: `MCTAS_BOT_OFFSET=-1,-1,0 mctas door vanilla-check
  designs/piston-door-3x3/archive/piston-door-3x3-rb2s2-50b.txt --extra-offsets 0` (DIVERGES at
  4/4 positions). With the bot floating free (touching no block) both designs match the
  simulator tick for tick. So in a real world these two sculk doors break when the player
  stands next to the lever on the sensor: exactly the risk the separate sculk board is for.
  Per-tick trace of both worlds (pending ticks): logs/rb2s2-50b-bot-on-sensor-trace.log.
* A lever popped by a broken build (fuzz case fuzz/fuzz-1034-error.txt) made the bot wait
  forever for it to flip; now the bot clicks the cell anyway (the judge's mock-player use on
  air does nothing either) and both worlds stay identical.

* `save-all` writes only chunks marked unsaved, and scheduling or running a block tick does not
  mark a chunk unsaved: a chunk whose blocks did not change keeps a STALE `block_ticks` list on
  disk. v2-115b looked "idle" 3 ticks early in vanilla (door not restored at 2/8 positions)
  until every snapshot first flips a marker block at y=-64 in each chunk (strict, no updates)
  so every chunk is re-saved. With fresh tick lists v2-115b matches tick for tick.
* The last position of a design was not cleared before the next design (positions overlap):
  now the previous box (plus a 6-block margin) is wiped, ticks/entities/block entities checked.
* 26.x world layout and palette encoding (see above); the server's default `white-list`.

## Results (2026-10-18, official server jar sha1 33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c)

No game divergence found. Every design below behaves IDENTICALLY in the unmodified vanilla
server and in the simulator: every block of the watch box, every tick, at every position, all
3 cycles, plus the real-time 20 tps cycle (open/close ticks are the judge's numbers).

All 32 judge positions (`--extra-offsets 28`, logs/C-bests-32pos.log, logs/E-new-records-32pos.log):

| design | blocks | category | ticks compared | open / close |
|---|---|---|---|---|
| fresh-c fc2-40b (inputs/) | 40 | solid | 4832 | 24 / 9 |
| fresh-c fc1-42b | 42 | solid | 4832 | 24 / 14 |
| fresh-b mp7-45b | 45 | solid | 3488 | 16 / 16 |
| fresh-b mp6-46b | 46 | solid | 3488 | 16 / 16 |
| m48-bmobs-47b | 47 | solid | 4832 | 22 / 25 |
| rb6-bmobs-47b | 47 | solid | 4832 | 22 / 25 |
| slime-free combo-48b | 48 | solid, slime-free | 8672 | 24 / 50 |
| logic-min-slime-free-50b | 50 | solid, slime-free | 8864 | 24 / 52 |
| mechanics fast-57b | 57 | solid | 3488 | 16 / 17 |
| rb9-61b | 61 | solid | 3392 | 16 / 17 |
| st43-bmobs-42b | 42 | sculk | 8288 | 22 / 38 |
| gravity g10-45b | 45 | gravity | 3950 | 5 / 19 |
| search-tool gravity 46b | 46 | gravity | 3950 | 5 / 19 |
| explore/gravity BEST (46) | 46 | gravity | 3950 | 5 / 19 |
| gravity-g8-48b | 48 | gravity | 3182 | 5 / 11 |
| gravity-g6-49b | 49 | gravity | 3182 | 5 / 11 |

8 positions (logs/B-older-8pos.log): v2-115b, fresh-b pd3-61b (standing torches), v2u2-88b
(honey), rb2-sculk-48b, m51-sculk-48b, gravity-g1-67b, obs-s1-89b, mechanics-rb2-hold-51b
(silent hold), rb7-71b, mechanics-ta-89b, logic-min-v2-49b, sf1-126b: all PASS identical.
The first batch also ran m48, rb6, logic-min-slime-free-50b and st43 at vanilla's own 20 tps
tick rate (8 positions): identical too, so `--tick-rate 100` changes nothing.

Every explore branch's BEST*.txt / FAST.txt at 8 positions (logs/F-branch-bests-8pos.log, copies
in inputs/branch-bests/): center-mech (48 sculk), fresh-a slime-free (122), fresh-b BEST (43
solid, open 16), fresh-c BEST (40), gravity BEST (45), logic-min BEST (49) and slime-free (50),
mechanics BEST (47), fast (57), sculk (42), slime-free (99), observer-seq (58), redstone-block
BEST (48) and FAST (61), riders (46), search-tool (88), slime-free (48), speed (114, open 12),
top-control (47, both): all 20 PASS identical.

Every archive design at 4 positions: logs/D-archive-4pos-part1.log (fresh-a .. m51-sculk, 25
designs) and logs/D-archive-4pos-part2.summary.log (the other 72): 89 PASS identical, the two
`v1-invalid` files FAIL identically in both worlds (slime door cell), and 6 "diverged", all
harness/placement effects explained below and PASS once handled (logs/H-recheck-fixed.log):
rb2s2-50b and rb2-sculk-50b (the bot stood on their sculk sensor), and four search-tool gravity
files (from-HEAD-gravity-g4-51b-46b/48b, g6-49b-46b/48b) that are not at rest as written (stale
dust powers, one piston the judge's self update retracts): vanilla-check now checks such a file
from its simulator rest state and says so.

Differential fuzz (`door vanilla-fuzz`, logs/G-fuzz*.log): random 6x5x6 contraptions from a wider
vocabulary (pistons, observers, repeaters, dust, torches, redstone blocks, lamps, targets, waxed
copper bulbs, note blocks, slime, honey, glazed terracotta, obsidian, sand), settled in the
simulator to a true rest state, a lever on a stone block touching a component, compared tick for
tick (2 cycles, one judge offset per seed): seeds 100-159 50 identical (10 never settle), seeds
1000-1029 29 identical + fuzz-1034 (the lever pops; the old harness waited for it forever, now
identical), seeds 2000-2079 59 identical + fuzz-2061 (a block pops, the bot standing nearby
PICKED UP the item so vanilla counted 0 entities vs 1: items in the bot's inventory now count as
entities, then identical; repro `door vanilla-check fuzz/fuzz-2061.txt --offset=-3,4,-11
--cycles 2 --max-ticks 120`), 20 never settle. No simulator divergence. Most cases are small
(5-20 ticks of activity), so this is a smoke test of the wider vocabulary, not a proof.

## Next ideas

* A "player build" check: place the design block by block with full updates in a sensible
  order (what a survival player can do) and compare the settled state with the file; designs
  that only exist through raw placement would show up.
* Fuzz with denser, door-like structures (pistons + slime around a 3x3 hole) and more cycles.
* `vanilla-check` is ~5 s per position with `--tick-rate 100` (one save-all + region read per
  tick); the coordinator could run it on every new record at the 32 judge positions.
