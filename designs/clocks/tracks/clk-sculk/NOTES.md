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
* calibrated + observer chains + emitter: 24-34 (3-7 blocks); + an empty dispenser (+4 gt): the
  cooldown pulse delayed 6 + 4 gt fires exactly at re-arm: P=21 in 5 blocks (c21-obs3-disp), the
  floor; plain: P=41 in 5 (p41-obs3-disp).
* calibrated + repeater lines (+ observer / torch / piston): 21-40.
* plain versions of both: 41-79.
* 3-sensor rings (calibrated): note-block resonator as the kick 31-46 (6-7 blocks); pure amethyst
  rings 30-46 (7-9 blocks, wool shields the kick); n-sensor rings on a circle: P = sum of hops,
  4 sensors 48-60, 5: 64-75, 6: 78-90, 8: 104-124, 12: 164-184 (2 blocks per <= 16 gt);
  plain rings need >= 6-7 sensors (47-56 with 14-16 blocks).
* hybrids with a sticky piston: period doubling (P 48 with 4 blocks, 88 with 4: the piston spits
  its block on alternate short pulses, so a trapdoor or a moved block toggles every other sensor
  cycle: the output must be that cell) and the moving-observer long periods (72, 85, 86, 90, 92,
  94, 126, 216 ... 468 with 4-7 blocks).
* dividers found by `clock census` (sculk palette, boxes up to 4x1x2 / 3x3x1 with k <= 4, 3x1x2
  with k <= 5): a fast observer pair (P 6) whose dispenser a plain sensor gates by powering it
  (P42 in 4 blocks: the sensor fires every 7th pair cycle); a calibrated c28 loop whose dispenser a
  plain sensor behind it also hears (P140 in 4 blocks: the plain sensor's 40 gt dead time divides
  the 28 gt loop 5:3; its power into the calibrated sensor's back is a frequency filter at 10, the
  dispenser's own frequency, so it passes); P23 in 4 blocks (a QC piston under an observer).
* The output rule (coordinator 16:10Z): a clock's period is its output's period. `mctas clock
  sculk outputs` measures each cell's own period: dividers and doublers declare their slow cell as
  the output; a design where no cell has the full period is a composite of independent parts
  (e.g. 30 and 20 -> 60) and is excluded (../composite/).

## Frequency filter that matters (filter/, 19:57Z)
A calibrated sensor hears only vibrations whose frequency equals its back signal (0 = all).
filter9-trapdoor: C watched by an observer whose pulses reach a trapdoor through a stone + dust;
a plain sensor S at C's back, sqrt(11) from the trapdoor, outputs 15 - floor(15 * 3.32 / 8) = 9
when it hears it: while S is active (30 gt) C ignores the trapdoor opening (block_open, 10) and
fires on its closing (block_close, 9) 2 gt later. S is deaf 40 gt, so it hears every other cycle:
cycles of 26 and 24 gt, P = 50 (redsim; the same build without S: P24). Same with a fence gate or
an iron trapdoor (open 10 / close 9). A note block or a dispenser (10 only) would make C deaf.

## Wool shutter (tried 20:27Z, not a clock of its own yet)
A sticky piston driven from C through two delay-4 repeaters pushes a wool block into the line from
an observer-pair trapdoor to C: redsim occludes as the game does (all 6 nudged lines must cross
wool) and the trapdoor events in the window are lost, but the piston's OWN retraction
(block_deactivate, 3 from C) is heard first, so the loop is really "C -> repeaters -> piston" and
the observer pair is an independent passenger (composite: C every 29, whole state 174). A real
shutter clock needs the piston out of C's range (> 16, powered through a longer line) or itself
behind wool, and an emitter coupled to C.

## Extendable families (one catalog card each; extend/README.md)
`mctas clock sculk family <name> --n <n>`: replin (P = 9n + 4, 3 <= n <= 14), hairpin (16n + 3,
n >= 2; plain n >= 3), racetrack (32m + 28; plain 16m + 12, m >= 3). Every P >= 21 is reachable:
a hairpin whose repeater delays sum to D rt plays its note 2D + 2 gt after the sensor fires
(>= 20 once D >= 9), so P = 2D + 3 with the note diagonal to the sensor (travel 1) and 2D + 4
with it one block further (travel 2).

## Open questions / next
* Exhaustive minimality: the census (k <= 4 in 3x2x2, 3x3x1, 4x1x2, 2x2x2; k <= 5 in 3x1x2) finds
  no 4-block P=21 or P=22 (self-start or one block placed last), and nothing below 3 blocks.
* Frequency-filter clocks where the filter matters (a back signal of 9 vs 10), wool shutters moved
  by pistons.
* Independent sub-clocks (lcm) are EXCLUDED (coordinator ruling 16:10Z).
