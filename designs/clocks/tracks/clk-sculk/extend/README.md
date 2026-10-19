# clk-sculk: extendable clocks (one catalog card each, vjeux 19:16Z)

A repeated unit; P = base + n * inc, blocks = b0 + n * db. Every design here is judged by
`mctas clock verify` (4 positions x 4 rotations, redsim agrees) before it is listed as verified.
Any n: `mctas clock sculk family <name> --n <n> --out-dir DIR` (crates/clock-sculk/src/family.rs).

| family | P | blocks | n range | unit (extension rule) | start | verified n (robust) |
|---|---|---|---|---|---|---|
| `replin` | 9n + 4 | 2n + 3 | 3..14 | one more delay-4 repeater on a stone in the line: +8 gt delay, +1 gt travel = +9 gt, +2 blocks | self | 3 (P31), 5 (P49), 8 (P76), 14 (P130) |
| `hairpin` | 16n + 3 | 4n + 7 | n >= 2 | one more delay-4 repeater on a stone in each arm: +16 gt, +4 blocks | self | see below |
| `hairpin-plain` | 16n + 3 | 4n + 7 | n >= 3 | the same with a plain sensor | self | see below |
| `racetrack` | 32m + 28 | 4m + 4 | m >= 1 | one more calibrated sensor + amethyst on each straight side, 17 apart: +2 hops of 16 = +32 gt, +4 blocks | kick (note block) | 1 (P60), 2 (P92), 4 (P156), 6 (P220) |
| `racetrack-plain` | 16m + 12 | 4m + 4 | m >= 3 | one more plain sensor + amethyst on each straight side, 9 apart: +2 hops of 8 = +16 gt, +4 blocks | kick (note block) | see below |

* `replin`: a calibrated sculk sensor, a straight line of n delay-4 repeaters read from it, an
  observer watching the last repeater, a note block behind the observer (n + 2 from the sensor).
  The line's turn-on plays the note 8n + 2 gt after the sensor fires (>= 20 for n >= 3: the sensor
  has re-armed), the vibration travels n + 2 gt. Bounds: n >= 3 (re-arm), n <= 14 (the note must
  stay within 16). n = 1, 2 are other regimes (P 23, 32: the cooldown edge is the one heard).
* `hairpin`: the same line folded: n repeaters away from the sensor, a two-dust U-turn, n
  repeaters back, an observer beside the sensor watching the last one, a note block diagonal to
  the sensor (travel 1 for every n): T = 16n + 2 >= 20 (calibrated, n >= 2) / >= 40 (plain,
  n >= 3), unbounded. n = 1 (calibrated: P29) / n = 1, 2 (plain: P49, P65) are other regimes.
* `racetrack`: calibrated sensors on two straight lines 18 apart, one turn sensor 13 beyond each
  end; each sensor's resonator (amethyst; the first a note block = the kick) sits on the face
  toward the next sensor. Line hops 16, turn hops 15: a sensor two hops ahead is always more than
  16 away, so only the next one hears. Unbounded in m (the box grows 17 per unit).
* `racetrack-plain`: plain sensors (radius 8): lines 10 apart, spacing 9 (hop 8), turn sensors 6
  out (hops 7); a sensor is deaf 40 gt, so the ring needs P - 8 >= 40: m >= 3.
