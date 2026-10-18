# sculk-2 track: sculk sensors replacing control wiring in the 40-block families

Goal: the Sculk board (designs containing a sculk sensor), well below 40, starting from fc2-40b,
riders-fx1-40b and m05-21b-sym-40b.

## Source facts (26.3, read with `mctas src show`)

* `SculkSensorBlock`: ACTIVE 30 gt then COOLDOWN 10 gt (scheduled ticks), power = 
  `max(1, 15 - floor(15/8 * d))` (d = distance to the vibration source, radius 8), weak power on
  every side, strong power only into the block BELOW (`getDirectSignal` UP). `activate` and
  `deactivate` update the sensor's neighbors AND the neighbors of the block below it.
  It has a block entity: immovable by pistons and never dragged by slime.
* `CalibratedSculkSensorBlock`: ACTIVE 10 gt (then cooldown 10), radius 16, no output into its
  back (the back is the frequency input: a non-zero back signal = only that frequency).
* Vibration timing (`VibrationSystem.Ticker`, `VibrationSelector`): an event at tick T is
  selected at T+1 (block-entity phase) and received after floor(d) ticks: activation in the
  block-entity phase of T + max(1, floor(d)). A sensor that is not INACTIVE ignores events; a
  vibration in flight blocks new ones. Same-tick candidates: the closest wins, ties to the higher
  frequency.
* Frequencies: lever on / piston extend = BLOCK_ACTIVATE (10); lever off / piston retract =
  BLOCK_DEACTIVATE (9). A piston extension that fails (blocked) emits nothing; a retraction
  always emits. Observers, repeaters and dust emit nothing.
* An observer watching a sensor fires 3 times per activation (inactive->active, active->cooldown,
  cooldown->inactive).

## Placement note (vanilla-check finding)
A player standing ON a sensor triggers it (stepOn) and can break a door. Every design here notes
whether a sensor is a cell a player could stand on next to the lever.

## Log

### 05:45 s1: 38 blocks, sculk (open 19 / close 38), `s1-38b.txt` = BEST.txt
`st43-bmobs-42b` (the rb family with a sculk hold on Q) fed its B window through a 7-block chain:
an observer watching the lever, a block, a dust and its floor, a block, then R_B (delay 4) into B's
quasi cell, and O' (observer watching R_B) pulsing A through the cross dust X' at (1,4,1).
s1 replaces the first 5 of those with ONE calibrated sculk sensor at R_B's input (1,6,2)
(`calibrated_sculk_sensor[facing=north]`, its back (1,6,3) empty so it hears every frequency):
it hears the lever 3 gt after each flip and is active 10 gt, so R_B (now delay 1) powers B over
t5..15 on both edges and O' still pulses A at t7 and t17 (R_B's two edges). Patch: `s1.patch`.

* Why delay 1: B must push A down after P_R has dragged T out of (1,3,0) (block event at t4), so
  the window starts at t5. With the normal 30 gt sensor the window would end after Q's hold
  (t33) and the closing T would hit A still down; the calibrated 10 gt window ends at t15.
* Loops: a sensor that is inactive again hears the door's own pistons. The calibrated sensor is
  inactive from t23; the open is done at t19 (no echo). On close Q's hold ends at t33 and wakes
  it again (t36): R_B re-powers B, but A is extended by then (blocked push) and O''s pulses go
  into an already powered A, so the echo moves nothing (idle at 56).
* Sensors and players: the hold sensor (0,3,-1) sits under Q (no headroom, nobody stands on
  it). The calibrated sensor (1,6,2) is 3 blocks from the lever with open air above it: a
  player could step on it, and like every calibrated sensor it hears footsteps within 16
  blocks; flip the lever from the west/north side, sneaking.
* `door minimize --moves all --plateau`: local minimum at 38 (28k neutral edits, 644 removals).
* Tried: dust+floor X' -> one block at (1,4,1) (A at (1,3,0) is then only quasi-powered and is
  never updated when the pulse ends: it stays extended and B cannot lift it); removing (0,3,0)
  (that block rides the left structure and, moving back at t1 of the close, is what makes the
  right structure's early pull fail).
* Before s1: `regrow --size 2 --any` (cut any 2 control blocks, add 1 sensor) on fc2, fc2-2,
  fc2-min, riders-fx1, m05-21b-sym: 1.2M candidates, nothing. `regrow --size 3` on fc2 with
  sensors + obs/block/dust/repeaters: 2.8M, nothing. fc2 with its B chain replaced by a sensor
  echoes forever (an observer on a sensor fires 3 times; the 3rd, at cooldown end, moves A,
  which the sensor hears again).

### 06:15 s4: 36 blocks, sculk (open 20 / close 37), `s4-36b.txt` = BEST.txt
s1 with the left pusher P_L moved down from (0,4,0) to (0,3,0) (it pushes slime (-1,3,0); the
slime (-1,4,0) and the rider block (0,3,0) go). P_L's own base, retracting at t1 of the close,
is now what makes the right structure's early pull fail (T would push C into (0,3,0), which is a
moving piston then). The lever's block K moves to (0,3,-1), the hold sensor's old cell: it
powers Q (above it) and P_L (south of it) directly; the lever hangs on K's west face (-1,3,-1)
(it is next to the left slime (-1,3,0): slime never drags a lever). The hold sensor moves next to
Q at (0,5,-1). Patch from s1: `s4.patch`. Variants with the hold sensor at (-1,4,-1) (s4b) or
(0,4,-2) (s4c) work too (36, 32 positions).

* Order that makes it work: the lever's update reaches Q first (K's neighbors in W,E,D,U,N,S
  order), Q's retraction moves RB and queues P_R's pull AFTER P_L's retraction, so P_L's base is
  already a moving piston when P_R tries to bring T back through C.
* Sensors and players: the hold sensor (0,5,-1) is two blocks above the lever, out of reach from
  the walkway floor; the calibrated sensor (1,6,2) is far above. Footsteps still wake both (the
  sculk category's known risk): a player walking through the open door, not sneaking, wakes the
  calibrated sensor and B then pushes A and C down into the opening.
* Tried first, failed: a silent Q (K quasi-powering Q so the lever never updates it; then no early
  pull and no blocker needed). The lever then sits above P_L's extended head, the head forwards
  the lever's update to P_L, P_L's retraction updates Q (they touch), and Q retracts at t1 anyway.
* `door vanilla-check s4-36b.txt --extra-offsets 28`: PASS, the official 26.3 server matches the
  simulator tick for tick (9344 ticks at 32 positions x 3 cycles; and a real-time 20 tps cycle at
  the default positions). The calibrated sensor behaves the same in vanilla.
* `door minimize --moves all --plateau` on s4: local minimum. `regrow --size 2 --any` and
  `--size 3` with both sensor kinds, observers, blocks, dust, repeaters, redstone blocks: nothing.
* `door verify --cycles 20`: fine. `door buildable`: BUILDABLE (36 actions, lever off while
  building, 6 placements against a temporary dirt block; the built door opens 20 / closes 37);
  build script `s4-36b.build.txt`. The plain order fails (placing R_B fires B early).
* `door minimize --moves all --plateau-pairs --depth 2 --walk 60` on s4: nothing.

## Budget of s4 (36)
door 9; left: P_L + 4 slime; right: P_R + 6 slime; center: A, B, Q, RB, P_M; sensors 2 (hold
next to Q, calibrated delay line at R_B's input); control 8: lever + K, R_B, O', X' dust + floor,
P_M's observer + block.

## Ideas that did not work (and why)
* A sensor next to a pull-type pulse piston (fc2's P_L, P_M): a 30 gt (or 10 gt) window makes it
  push on the close edge and PULL BACK at the window's end; the toggle needs a 2 gt pulse (spit).
* An observer on a sensor: 3 pulses per flip (wake, cooldown, inactive); the third comes after the
  sensor is listening again, so whatever it moves wakes it again (s1-era test: fc2 with its B chain
  on a sensor "still moving 400 ticks after opening").
* Sensor-driven B in fc2's geometry: every cell next to B or its quasi cell is 3-4.5 blocks from
  the lever, so the window starts before T is aside; B's blocked tries are never retried.
* fc2/riders + hold + calibrated B: 39 at best. The K-direct Q (which saves Q's repeater and its
  floor) needs the lever next to Q, and then fc2's P_L observer chain or riders' p0 observer cannot
  reach it.
* Silent Q (no early pull at all): no lever/K placement quasi-powers Q and P_L without updating
  one of them (P_L's head or base touches Q's neighbours).
* One sensor for both the hold and B's window: a normal window makes B retract after Q's hold
  ends (T returns while A is down); a calibrated hold ends before the dance.
* X' as one block (no dust): A at (1,3,0) is only quasi-powered and not updated when the pulse
  ends, so it does not spit C on close; C comes back up and the last pull parks it on T's path.

## Next ideas
* A Q-less center: RB riding the left structure to power A at rest, P_R on a sensor window whose
  open-edge pull-back fails against P_L's extended base (it does: C pushed into it). Blocker: A
  gets RB back too early on close unless P_L holds, which needs a delayed K path.
* Frequency filter for free: a calibrated sensor whose back touches K hears nothing while the lever
  is on (filter 15), i.e. a close-only window. Not needed by s4, but it gives asymmetric control.
* P_M's trigger (2 blocks) is the last non-shared control pair; an inverted P_M (extended at rest)
  powered by a block riding a structure costs the same or more here.

## Round 2 (coordinator's ideas, 06:50-)

### (1) Q-less: a redstone block riding the left structure powers A at rest
Blocked by geometry in this family, and the searches agree:
* A's free neighbors are (0,4,0) and (1,4,-1). Only (0,4,0) can ride the left structure (with a
  slime at (-1,4,0)), but it sits right on top of P_L (0,3,0) and powers it at rest. With P_L
  back at (0,4,0) (s1) A has no free neighbor on the left at all; QC through (1,5,0) puts the
  block next to B (B fires at rest). A pull-type P_L from the west (fc2 style) frees (0,4,0) but
  costs its observer pair: about 38.
* Timing, even with a free spot: with A's power tied to the left structure, P_R must push at
  least 3 gt after the left structure leaves (open) and pull back no later than it returns
  (close). Lever-driven or observer-driven delays are the same on both edges, a sensor window
  makes the close LATER than the open (the window's end pulls), so P_R needs exactly the
  asymmetry Q's shuttle gives (RB lands 2 gt after Q moves, leaves at once). The close-only
  calibrated sensor (back on K) gives a close-only window but still a late one.
* `door grow` on s4 minus {Q, RB, hold sensor} (33 blocks), adding any 2 of: both sensor kinds
  (4 facings), redstone block, slime, observers, blocks, repeaters 1-2 in x -2..3, y 2..7,
  z -2..2: 2.2M candidates, none works. So no 35 this way.

### (2) Sharing P_M's 2-block trigger
* The trigger needs a 2 gt pulse (a sensor window makes P_M pull M back down when it ends) once
  per edge. The observer must face a cell that moves once per edge and output into P_M or its
  QC cell (1,-1,0); every cell next to P_M or (1,-1,0) faces only empty cells, so the observer
  needs its relay block. O' (two pulses per edge) and an observer on a sensor (three) fire
  P_M too often.
* `door grow` on s4 minus {P_M, O_m, block} (33), adding any 2 of a sticky piston facing up,
  observers, blocks, redstone blocks, slime, sensors, repeaters in x -1..3, y -4..-1, z -2..2:
  1.1M candidates, none works.

### Other round-2 checks
* `minimize --moves all --plateau-pairs` on the s4b/s4c variants: 36, no gain.
* Synthesis now knows sculk sensors (tool commit): `door synth`/`resynth` treat a sensor as a
  wireless source (fed by the lever, weak power on every side, a calibrated one not into its
  back, strong into the block below; observers can watch it), so sensors can be synthesized
  into any circuit. `resynth --max-size 4` on s4 with sensors in the palette: nothing (13 groups).
