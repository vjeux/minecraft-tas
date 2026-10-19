# clk-sculk: vibration clocks (sculk sensors, calibrated sensors, amethyst)

Track notebook. Every period below was MEASURED in the real game (`door drive`, the JVM), never
taken from memory. Designs and their drive scripts sit next to this file.

## The floor (from the 26.3 source, `crates/redsim/src/sculk.rs`)
A sensor activates only by receiving a vibration, and only an INACTIVE sensor with no vibration in
flight takes one. After activating at tick A it is INACTIVE again in the tile-tick phase of A+20
(calibrated: 10 active + 10 cooldown) or A+40 (plain: 30 + 10). A vibration posted at tick T
becomes the sensor's in-flight vibration at T+1 and lands at T + max(1, floor(distance)).
So a clock in which a sensor fires once per period has P >= 21 (calibrated) / P >= 41 (plain);
firing k times per period needs P >= 21k / 41k. Periods below 21 are impossible for this family.

## Clocks found (real game, door drive)
| P | blocks | file | start | how |
|---|---|---|---|---|
| 21 | 6 | c21s-rep-obs-note.txt | self | C strongly powers the stone below it; repeater (4) reads that stone; observer above the repeater plays a note block diagonal to C (dist sqrt2, travel 1). Event at A+20 (observer tick after C's INACTIVE tick: same priority, scheduled later) |
| 21 | 6 | c21-rep-obs-note.txt | kick | same, repeater placed unpowered |
| 22 | 5 | c22s-rep-obs-note.txt | self | C -> repeater (4) -> observer watching it -> note block at dist sqrt5 (travel 2) |
| 24 | 3 | c24-obs-note.txt | kick | C, observer watching C, note block: the observer's 3rd pulse (C's COOLDOWN->INACTIVE at A+20, pulse A+22) plays the note at dist 2 |

Self-start trick: a repeater placed `powered=true` with no input turns off on the placement's
self update; that edge is the first pulse. A stuck-powered observer never re-arms (it only
starts a signal while unpowered), so c24 needs a kick.
