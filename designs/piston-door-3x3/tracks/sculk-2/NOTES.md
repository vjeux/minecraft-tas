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
