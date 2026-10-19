# clk-sculk: extendable clocks (one catalog card each, vjeux 19:16Z)

A repeated unit; P = base + n * inc, blocks = b0 + n * db. Every design here is judged ROBUST by
`mctas clock verify` (4 positions x 4 rotations, redsim agrees). Any n: `mctas clock sculk family
<name> --n <n> --out-dir DIR` (crates/clock-sculk/src/family.rs).

| family | P | blocks | n range | unit (extension rule) | start | verified n |
|---|---|---|---|---|---|---|
| `replin` | 9n + 4 | 2n + 3 | 3..14 | one more delay-4 repeater on a stone in the line: +8 gt of delay, +1 block of travel = +9 gt, +2 blocks | self (repeaters placed powered) | 3 (P31), 5 (P49), 8 (P76), 14 (P130) |
| `racetrack` | 32m + 28 | 4m + 4 | m >= 1 | one more calibrated sensor + amethyst on each straight side, 17 apart: +2 hops of 16 = +32 gt, +4 blocks | kick: right-click the first sensor's note block | 1 (P60), 2 (P92), 4 (P156), 6 (P220) |

* `replin`: a calibrated sculk sensor, a straight line of n delay-4 repeaters read from it, an
  observer watching the last repeater, a note block behind the observer (n + 2 from the sensor).
  The line's turn-on plays the note 8n + 2 gt after the sensor fires (>= 20 for n >= 3: the sensor
  has re-armed), the vibration travels n + 2 gt. Bounds: n >= 3 (re-arm), n <= 14 (the note must
  stay within 16). n = 1, 2 are other regimes (P 23, 32: the cooldown edge is the one heard).
* `racetrack`: calibrated sensors on two straight lines 18 apart, one turn sensor 13 beyond each
  end; each sensor's resonator (amethyst; the first a note block = the kick) sits on the face
  toward the next sensor. Line hops 16, turn hops 15: a sensor two hops ahead is always more than
  16 away, so only the next one hears. Unbounded in m (the box grows 17 per unit).
