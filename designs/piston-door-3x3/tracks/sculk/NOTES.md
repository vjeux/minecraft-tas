# sculk track: sculk sensors in redsim, then a 7x8 / 7x9 flat door with sculk cross-door links

Brief (coordinator, vjeux 05:20Z + order change 05:23Z): sculk sensors are allowed (their own
board). A sensor hears VIBRATIONS (game events: a piston extending/retracting, a block destroyed...)
within 8 blocks (calibrated: 16, with a frequency filter on its back), wirelessly: the missing link
across a 7-wide flat door (only 2 columns per side). Order: (1) model sculk EXACTLY in redsim,
validated by a pistfuzz-style differential fuzz against the real game; (2) sculk in cosearch's
palette; (3) design / search the 7x8 with sculk links in redsim, the JVM judge only at the end.

## 1. The model: `crates/redsim/src/sculk.rs` (read in the 26.3 source)

* Block `Block::Sculk { calibrated, facing, phase, power }` (`minecraft:sculk_sensor`,
  `minecraft:calibrated_sculk_sensor`; phase 0 inactive / 1 active / 2 cooldown).
* Game events: every `World::game_event*` call is posted to the sensors (`sculk::dispatch`):
  in range when `distSqr <= r^2` (r = 8, calibrated 16); ignored while a vibration is in flight;
  the event must be a vibration (frequency > 0: `sculk_sensor_tendrils_clicking` is not); an
  affected state that dampens (wool) drops it; `canReceiveVibration` (calibrated: a non-zero back
  signal `getSignal(pos + back, back)` admits only that frequency; never the sensor's own
  destroy/place; INACTIVE only); not occluded (`isOccluded`: the 6 lines from the source center
  nudged 1e-5 toward each face all cross wool, an exact port of `BlockGetter.traverseBlocks`).
  Then the event is the sensor's selector CANDIDATE stamped with the game time: a later event of
  the same tick replaces it only if closer (tie: higher frequency); a later tick never replaces it.
* Block-entity phase (`VibrationSystem.Ticker`, sensors in creation order, after hoppers, before
  moving blocks): with nothing in flight, a candidate from an EARLIER tick becomes the vibration
  (travel = floor(distance)); travel - 1; at 0 it is received: an INACTIVE sensor sets
  `last_freq`, activates with power `max(1, 15 - floor(15/r * d))` (setBlockAndUpdate, a
  scheduled tick 30 gt / calibrated 10 gt, then its neighbors and the neighbors of the block
  below are updated again), and every adjacent amethyst block emits `resonate_<freq>`
  (Direction.values() order). Every data change (`onDataChanged` -> `setChanged`) updates the
  comparators around the sensor. So an event at tick T activates the sensor in the block-entity
  phase of tick T + max(1, floor(d)).
* Scheduled tick: ACTIVE -> COOLDOWN (power 0, 10 gt, neighbors + below updated again), COOLDOWN
  -> INACTIVE. `onPlace`, `affectNeighborsAfterRemoval` as in the game.
* Signals: weak `power` on every side (calibrated: not toward its back), strong only into the
  block below; signal source (dust connects); not a conductor; only the bottom face is sturdy;
  analog output = `last_freq` while ACTIVE; a block entity (pistons cannot move it).
* Event sources added to redsim: `Level.destroyBlock` (block_destroy, with the destroyed state),
  trapdoors toggled by redstone (block_open/close, before the write), empty dispensers firing
  (block_activate); pistons (extend/retract/destroy), levers, buttons, note blocks, doors and
  fence gates already recorded theirs.
* `World::idle` ignores vibrations in flight, like the bridge's idle check (`Sandbox.idle`):
  the judge can call a door settled while a vibration still travels (keep links short or keep
  something else pending; `sculk::State::busy` tells).
* Not modeled: entities (an ITEM ENTITY LANDING is a `hit_ground` vibration: any drop breaks
  exactness near sensors), players (footsteps: `step`, frequency 1), sensors created mid-run
  (their ticker would come after live moving blocks; designs place every sensor first).
* `classes.rs`: wool (occludes, dampens) and amethyst (resonates) are no longer equivalent to
  other plain blocks (`Signature::vibration`).

## 2. The fuzz: `mctas door pistfuzz --sculk [--border]`

`gen_sculk` (pistfuzz.rs): flat 13x10x1 or 10x7x6 boxes, calm (8% pistons, few redstone blocks)
or busy; plain and calibrated sensors, wool, amethyst, note blocks, trapdoors, empty dispensers,
redstone blocks, observers, lamps; amethyst next to sensors, wool shells around some; comparators
reading sensors (+ dust), observers watching them, dust; frequency filters (a dust line from a
redstone block or a floor lever into a calibrated sensor's back: back signal 8..15); inputs over
130 ticks: redstone blocks set/cleared near pistons, lever clicks, wool/stone placed, extended
piston bases removed (their heads break: block_destroy without a drop). With a sensor in the
scenario, the comparison stops after the first frame with a drop (`compare_upto`).

| run | identical | mismatching | ticks compared | activations |
|---|---|---|---|---|
| `--sculk --count 3000` (05:55Z) | 2893 | 0 | 209,569 | 159,223 |
| `--sculk --border --count 300` (sensors in the sandbox's edge chunks) | 285 | 0 | 19,663 | 14,492 |
| `--designs` all 16 archived sculk doors, 3 cycles | 16 | 0 | 4,462 | 186 |
| `--gadgets` (81 drive scripts, 4 sculk gadgets) | 81 | 0 | 1,997 | 13 |
| `--mixed --observers --shapes --count 400` (regression, no sensors) | 396 | 0 | 19,008 | - |

(skipped runs: a scripted lever click on a cell a piston replaced: `use on stone`.)
First run (before the drop rule): 18 of 39 mismatched, every one after a drop: the game's item
entities landing (`hit_ground`) woke sensors.

## Log
* 05:23Z start; 05:42Z model + fuzz; 05:55Z 0 mismatches in 3000.
