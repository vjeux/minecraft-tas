# clk-sculk: vibration clocks (sculk sensors, calibrated sensors, amethyst)

Track notebook (session 0d1a0fb5). Every clock here was judged by `mctas clock verify` (the real
game, 8 positions x 4 rotations) unless marked "sim only". Generators: `mctas clock sculk ...`
(crates/clock-sculk). Board: Space memory `minecraft-tas/clocks/families/sculk.md`.

## The floor (from the 26.3 source, `crates/redsim/src/sculk.rs`)
* A sensor activates only by RECEIVING a vibration, and only an INACTIVE sensor with no vibration in
  flight takes one (the event is ignored otherwise: nothing queues).
* Activated at tick A (block-entity phase), it is INACTIVE again in the tile-tick phase of A+20
  (calibrated: 10 active + 10 cooldown) or A+40 (plain: 30 + 10).
* A vibration posted at T becomes the in-flight vibration at T+1 (the `t < now` rule) and lands at
  T + max(1, floor(distance)); radius 16 calibrated, 8 plain.
* => a clock in which a sensor fires once per period has **P >= 21 (calibrated) / P >= 41 (plain
  only)**; k firings per period need P >= 21k / 41k. Periods 1..20 are impossible for any clock
  whose active set holds a sensor; 21..40 need a calibrated one. Both floors are reached (c21s*,
  p41s*).
* No block reacts to the sensor's COOLDOWN->INACTIVE step except an observer watching it, so every
  single-sensor clock is "sensor + delay + emitter", and the event that re-fires it is the first
  emitter event at or after A+20 / A+40.
* Placing blocks makes no vibration (bridge and redsim): a sculk clock self-starts only through a
  block whose placement update moves it: a trapdoor / fence gate placed open+powered with no power
  closes, a piston placed extended without power retracts, a repeater placed powered with no input
  turns off, a note block placed beside a lit torch plays. Otherwise it needs a kick.

## Vibration sources driven by redstone (read in the source)
note block (rising edge, needs air above, freq 10), piston / sticky piston (extend 10, retract 9,
block-event phase), trapdoor / iron trapdoor / fence gate / door (open 10, close 9, at the power
change), EMPTY dispenser (block_activate 4 gt after a rising edge; a dropper or crafter makes NO
game event), bell (freq 11, not in redsim), amethyst block (resonates the received frequency when
the sensor beside it activates). Observers, repeaters, comparators, dust, lamps, copper bulbs,
torches, hoppers make none.

## Timing rule of a single-sensor clock
The sensor's own events: ON at A, OFF at A+10 (A+30 plain), INACTIVE at A+20 (A+40, visible to an
observer only). A delay d after them gives emitter events at A+d, A+10+d, A+20+d (+2 for an
observer); the first one at >= A+20 (A+40) re-fires the sensor after floor(distance):
P = T + travel. Observer chains of k: T = 20 + 2k for k <= 4 (k = 5 brings the SECOND pulse to A+20:
P drops), so observer-only clocks stop at T = 28; repeaters before an observer give T = 12 + d for
d in [8, 18) and d + 2 beyond; a note block hears only the rising edge, so a repeater line into a
note block gives T = d for d >= 21 (a repeater's tile tick runs before the sensor's own INACTIVE
tick in the same tick, so exactly A+20 needs an observer, torch or piston).

## Clocks (all ROBUST 32/32 in the real game; designs in this folder)
| P | blocks | file | start | mechanism |
|---|---|---|---|---|
| 21 | 6 | c21s-rep-obs-note | self | sensor strongly powers the stone under it; repeater (4) reads that stone; observer above it plays a note diagonal to the sensor (travel 1) |
| 21 | 6 | c21s-rep-torch-note | self | repeater (4) -> stone -> wall torch relights at A+20 -> note block |
| 21 | 10 | c21s-rep-dust-piston | self | repeater line -> piston behind the sensor: RETRACTION exactly at re-arm |
| 22, 23 | 5 | c22s-, c23s-rep-obs-note | self | sensor -> repeater (4) -> observer -> note (travel 2 / 3) |
| 24 | 3 | c24s-obs-trapdoor / irontrapdoor / fencegate / pistonext, c24-obs-note / piston / sticky | self / kick | sensor, observer watching it, emitter behind the observer: the observer's 3rd pulse (cooldown -> inactive) |
| 25-27 | 4 | c25-obs-stone-note, c26-obs2-note, c27-obs2line-note (+ c27-obs3curl-note 5b) | kick | 1-2 observers, travel 2-3 |
| 28-31 | 3-4 | c28-obs-dispenser, c29-obs-stone-disp, c30-obs2-disp, c31-obs2line-disp | kick | empty dispenser: +4 gt |
| 41 | 6 | p41s-rep-obs-note | self | plain-sensor floor |
| 42, 43 | 5 | p42s-, p43s-rep-obs-note | self | plain |
| 44 | 3 | p44-obs-note | kick | plain sensor + observer + note |
| 45 | 7 | ring3-c45 | kick | PURE vibration ring: 3 calibrated sensors + 3 amethyst (only sensors change state), hops 15 |
| 48 | 3 | p48-obs-dispenser | kick | plain + observer + empty dispenser |
| 72, 90, 126 | 6, 6, 7 | c72-, c90-, c126-obs-moving-sticky | kick | hybrid: a sticky piston beside the observer chain's output pulls the sensor-watching observer in and out: a fast observer-piston oscillator gated by the sensor's dead time |
| 216 | 6 | p216-obs-moving-sticky | kick | the same with a plain sensor |

Generated and verified batches: `verified/` (see its README).

## Families and what reaches which periods (so far)
* calibrated + observer chains + emitter: 24-34 (3-7 blocks).
* calibrated + repeater lines (+ observer / torch / piston): 21-40.
* plain versions of both: 41-79.
* 3-sensor rings (calibrated): note-block resonator as the kick 31-46 (6-7 blocks); pure amethyst
  rings 30-46 (7-9 blocks, wool shields the kick); n-sensor rings on a circle: P = sum of hops,
  4 sensors 48-60, 5: 64-75, 6: 78-90, 8: 104-124, 12: 164-184 (2 blocks per <= 16 gt);
  plain rings need >= 6-7 sensors (47-56 with 14-16 blocks).
* hybrids with a sticky piston: period doubling (P 48 with 4 blocks, 88 with 4) and the
  moving-observer long periods (72, 85, 86, 90, 92, 94, 126, 216 with 4-7 blocks).

## Open questions / next
* Exhaustive minimality (P=21 at 6 blocks? 25-27 at 4?): `clock census` with a sculk palette.
* Frequency-filter clocks (a calibrated sensor's back input selecting 9 vs 10), wool shutters
  moved by pistons, bells and doors as emitters.
* Independent sub-clocks multiply: two non-interacting clocks in one build have P = lcm (e.g. 24
  and 25 -> 600 with ~7 blocks): a rules question for the coordinator, not pursued.
