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
* 06:20Z cosearch palette (`--palette all,sculk,calibrated,wool,amethyst`), redsim judge vs JVM
  on 6 sculk doors + 90 mutants: 96/96 identical (`cosearch agree`).
* 06:34Z `door ideal-rs` (redsim `door ideal`); 07:05Z merged main (door CLI split: ideal-rs now
  lives in crates/door-redsim, its loop in piston-door/ideal_rs.rs).

## 3. `door ideal-rs`: timing windows in milliseconds

`door ideal-rs <plan.ideal> [--set v=n] [--sweep v=lo..hi]... [--independent] [--trace]`: the
`door ideal` file format and checks (closed at rest, open, back closed, no drops) on redsim at the
judge's 4 positions; a grid sweep runs in parallel (a 40x40 grid in seconds). Agrees with the
JVM's `door ideal` on m10a/m10/m8/m9. Windows measured with it (plans in `seeds/`, `m10b.*`):

* **u7-m10a has a SYMMETRIC train** (`seeds/m10s.ideal`: the same pulses after both flips, the
  lever's levels as in m10a): A hold start 0..2 for 3..4 gt (0..6 with start 0), K 5..10 with
  K >= U1 + 3, U1 2..6, U2 >= 8, L0 and L1+L2 any, A's last pull >= 12; open 15 / close 15.
* u7-m9 with BR split from MR (`seeds/m9L.*`, lever (-3,4)): BR free (0..40); U1 0..14 with
  K in [U1 + 3, 21]; a 10 or 30 gt hold instead of any bottom pulse breaks it (K: only 13 with a
  10 gt hold).
* ring56 t10's elevator compresses: A at a, a+10, a+20 works with E a+3, B a+6 and a+13..16
  (`seeds/t10v.ideal`, every elevator event its own variable) -- the spacing of the three state
  changes of a CALIBRATED sensor (an observer on it fires at T+2, T+12, T+22).
* m10b's K tolerates only short holds: k 7..8 (2 gt), 6 (3..4 gt), 5 (4..5 gt); never a sensor
  window.

## 4. What a sensor gives a flat door (all measured in redsim, `cosearch trace` / ideal-rs)

* Activation: an event at tick t activates a sensor at t + floor(d). Every cell of a 7x8 box is
  within ~8.5 of the lever, so every sensor that hears the lever wakes by t8 (plain range 8:
  cells farther than 8 hear the first in-range piston instead).
* Outputs per activation at T (w = 30 plain / 10 calibrated):
  - direct power: a window [T, T+w] (and strong power into the block below);
  - observer on the sensor: 3 taps (T+2, T+w+2, T+w+12);
  - sensor -> redstone lamp -> observer: 2 taps (T+2, T+w+6), no third;
  - sensor -> waxed copper bulb -> comparator: a LEVER-LIKE LEVEL that flips once per activation
    at T+2 (`tflipflop.txt`: sensor + bulb + comparator + its support, traced in redsim with
    `cosearch trace`); an observer on the comparator: 1 tap at T+4;
  - comparator reading the sensor: 9 or 10 (the lever's off/on frequency) for w ticks.
* **The echo rule.** A sensor re-arms at T+w+10 (plain T+40, calibrated T+20) and then hears the
  door's next event. Observer-on-sensor taps always echo (the third tap comes from the re-arm and
  moves a piston in range). Calibrated sensors re-arm before any 7x8 cascade with an elevator ends
  (30-45 ticks): only a short door (m10: 15 ticks) or plain sensors with T >= 6 stay quiet.
* **A window on a pull-type piston fails on the close**: the window's end re-pulls the block it
  just pushed home. Windows only suit pistons whose block has moved on by then.
* **Wool doors shield**: with wool door blocks the three static frame blocks are wool too; e.g.
  (2,-1) occludes R0's vibrations from (0,-2) and (3,2) occludes the lever (4,5) from the bottom.
* Edge-selective sensors (sculk-2's trick): a calibrated sensor whose back touches a block the
  lever powers hears nothing while the lever is on (filter 15): open-only for a lever-ON door.
* Geometry of the bottom corners: in the bottom row (y = -2) nothing needing support can stand
  (dust, repeater, comparator: y = -3 is outside), so a corner has observers, lamps, bulbs,
  sensors and blocks only; m10's corner piston K (-1,-1) can be fed only by dust (-2,-1) on a
  block powered by an observer at (-1,-2) watching (0,-2), or by (-1,-2) itself, and L0 only from
  (-2,-1): I found no way to feed both K and L0 there.
* One game tick: every vanilla delay except dust (0) is even (repeater, comparator, observer,
  torch: 2 gt steps; lamp off 4); a sensor adds floor(d), so it is the only odd delay (the
  coordinator's m79q, one tick short on B's chain, is the case). In m10b an observer on U at
  (0,-2) feeding K through a stone at (-1,-2) fires K at t13 and t17 (U's first pulse is blocked
  by the A hold's column; U moves only at t11): far from K's 7..8.

## 5. Designs tried

* **m10b** (`m10b.txt` + `m10b.ideal`, WORKS in redsim and in the game (`door ideal`, JVM: 4
  positions x 2 cycles, open 15 / close 15) with A hold, K, L0, L1+L2, A's last pull
  ideal): U wired in-plane WITHOUT sculk: observer (4,-1) watching R0 -> lamp (4,-2) (on at t3,
  off at t9) -> observer (3,-2) -> stone (2,-2) -> U: two pulses (t5, t11) on both edges; on the
  open the t5 one is blocked (the A hold has pushed the column onto U's head cell), so U moves
  once at t11, while the close pushes at t5 and t11. Left: K at
  7..8 and L0 in the bottom-left corner (see section 4), the A hold (lever-bound: starts 0..2).
* t10 + a calibrated sensor at (-2,4) lighting a lamp for A's observer (`t10-sk1.*`): the sensor
  QC-powers TL's puller (-2,2) through (-2,3) and re-extends it; with the sensor elsewhere the
  calibrated re-arm (T+20) echoes into A. Not wirable this way.
* cosearch with the sculk palette (all 25-min runs, 40 threads, ~150k screens/s): m10a (lever
  levels real, the rest assisted) best 75/79 with 1 assist / 73/79 with none; m10s 75/79; t10s
  69-75/79 (2 assists); m9L 73/79; m10s's bottom (top pinned) 71/79; m10b's K (5 cells,
  enumeration k=3 + anneal) no find; far sides with the elevator pinned (`--pin-assist`, 20 min):
  t10s 75/79 with MR+BR still assisted, t8L 75/79 with ML+BL still assisted (both partials place
  calibrated sensors, t8L's with a bulb: `finds-kept/`), no find.

## 6. Next ideas

* An edge-aware (oc) plan per mechanism + open-only/close-only calibrated sensors (back touching
  a lever-powered block): the asymmetric top ring56 wanted for t10's BM.
* A short mechanism (m10-like, 15 ticks) keeps every sensor echo-free; give it a bottom whose
  corner piston has a feed in the 2-row bottom (a variant where BL leaves down, as in t10).
* Level-type far-side pistons driven by one T flip-flop (sensor + bulb + comparator) placed in
  row -1 with its support in row -2.
* cosearch macro moves that place a whole gadget (sensor+lamp+observer, sensor+bulb+comparator)
  instead of one block at a time; an echo-aware score (a sensor's activations per flip).
