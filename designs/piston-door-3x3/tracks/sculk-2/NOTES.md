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
