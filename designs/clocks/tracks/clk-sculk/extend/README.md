# clk-sculk: extendable clocks (one catalog card each, vjeux 19:16Z)

Each card here (`<family>.txt`) is the family's base design with the `extend` headers
(docs/clocks.md "Extendable clocks"); `mctas clock archive` judged n = low, low+1, low+2, the
middle and the high end in the game (16 builds each, all robust) and recorded them in
`designs/clocks/archive/extend.tsv`. Any n: `mctas clock sculk family <name> --n <n> --out F`
(crates/clock-sculk/src/family.rs).

| family | P (gt) | blocks | n range | unit (extension rule) | start | judged n |
|---|---|---|---|---|---|---|
| `clk-sculk-replin` | 4 + 9n | 3 + 2n | 3..14 | one more delay-4 repeater on a stone in the line: +8 gt delay, +1 gt travel | self | 3, 4, 5, 8, 14 (P 31..130) |
| `clk-sculk-replin-trapdoor` | 4 + 9n | 3 + 2n | 3..14 | the same, an oak trapdoor as the emitter | self | 3, 4, 5, 8, 14 |
| `clk-sculk-replin-dispenser` | 8 + 9n | 3 + 2n | 2..14 | the same, an empty dispenser as the emitter (+4 gt) | self | 2, 3, 4, 8, 14 (P 26..134) |
| `clk-sculk-hairpin` | 3 + 16n | 7 + 4n | 2..12 (unbounded) | one more delay-4 repeater on a stone in each arm | self | 2, 3, 4, 7, 12 (P 35..195) |
| `clk-sculk-hairpin-trapdoor` | 3 + 16n | 7 + 4n | 2..12 (unbounded) | the same, an oak trapdoor as the emitter | self | 2, 3, 4, 7, 12 |
| `clk-sculk-hairpin-dispenser` | 7 + 16n | 7 + 4n | 1..12 (unbounded) | the same, an empty dispenser as the emitter (+4 gt) | self | 1, 2, 3, 6, 12 (P 23..199) |
| `clk-sculk-hairpin-plain` | 3 + 16n | 7 + 4n | 3..12 (unbounded) | the same, plain sensor | self | 3, 4, 5, 7, 12 (P 51..195) |
| `clk-sculk-racetrack` | 28 + 32n | 4 + 4n | 1..8 (unbounded) | one more calibrated sensor + amethyst on each straight side, 17 apart | kick (note block) | 1, 2, 3, 4, 8 (P 60..284) |
| `clk-sculk-racetrack-plain` | 12 + 16n | 4 + 4n | 3..10 (unbounded) | one more plain sensor + amethyst on each straight side, 9 apart | kick (note block) | 3, 4, 5, 6, 10 (P 60..172) |

* `replin`: a calibrated sculk sensor, a straight line of n delay-4 repeaters read from it, an
  observer watching the last repeater, a note block behind the observer (n + 2 from the sensor).
  The line's turn-on plays the note 8n + 2 gt after the sensor fires (>= 20 for n >= 3: the sensor
  has re-armed), the vibration travels n + 2 gt. Bounds: n >= 3 (re-arm), n <= 14 (the note must
  stay within 16). n = 1, 2 are other regimes (P 23, 32: the cooldown edge is the one heard).
* `hairpin`: the same line folded: n repeaters away from the sensor, a two-dust U-turn, n
  repeaters back, an observer beside the sensor watching the last one, a note block diagonal to
  the sensor (travel 1 for every n): the note plays 16n + 2 gt after the sensor fires, >= 20
  (calibrated, n >= 2) / >= 40 (plain, n >= 3); unbounded. n = 1 (calibrated: P29) and n = 1, 2
  (plain: P49, P65) are other regimes. With other delays (sum D rt) P = 2D + 3: every P >= 21.
* `racetrack`: calibrated sensors on two straight lines 18 apart, one turn sensor 13 beyond each
  end; each sensor's resonator (amethyst; the first a note block = the kick) sits on the face
  toward the next sensor. Line hops 16, turn hops 15: a sensor two hops ahead is always more than
  16 away, so only the next one hears. The box grows 17 per unit.
* `racetrack-plain`: plain sensors (radius 8): lines 10 apart, spacing 9 (hop 8), turn sensors 6
  out (hops 7); a sensor is deaf 40 gt, so the ring needs P - 8 >= 40: m >= 3.

Not cards: the 3-sensor rings (`ring3*`, P 30..46 in 6..9 blocks) are tuned by hop DISTANCES
(blocks constant), and the circle rings (`ringbig*`) have periods set by rounding on the circle
(not linear in n); both stay individual clocks.
