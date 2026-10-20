# Track pulsemult (study 11, fly): bursts, dividers, gated pulse trains

Branch explore/pulsemult. The timer half (monostable, off-delay, on-delay) is clk-rt's
(explore/timer, designs/circuits/pulsemult/tracks/timer/). Variations and results:
[VARIATIONS.md](../../VARIATIONS.md). Judge: crates/circuit/src/pulsemult.rs, docs/circuits.md
"Study 11 timers" (clockinfra reviewed every change).

## Archived (designs/circuits/pulsemult/archive/, all robust in the game at 16 builds)

| design | kind | line |
|---|---|---|
| pm-burst2-bulb | burst 2 | 7 blocks: bulb, comparator, observer, delay-3 repeater, observer; period 6, min-pulse 1 |
| pm-burst2-chase, pm-burst3-chase | burst 2, 3 (group) | 9 blocks: the bulb edge + a repeater hold the observer-chase clock's piston out; period 4; N by the repeater's delay |
| pm-burst3-bulb | burst 3 | 12 blocks, latency 12 (the chase one has 14) |
| pm-burst4-taps | burst 4 | 19 blocks (local search from three repeater taps) |
| pm-div3-square, -5-, -7- | divider 3, 5, 7 (group) | 13 / 16 / 23 blocks, 50% duty for odd N: study 5's ring + bulb fed on both edges; min-period 20 gt |
| pm-train-observers | pulse-train | 3 blocks, P 4 gt: a sticky piston pushes an observer face to face with another |
| pm-train-comparator-d1..d4 | pulse-train (group) | 12 blocks, P 8 / 12 / 16 / 20 gt: a subtract comparator, its output looped to its side through a repeater |

Linked, not archived: every study-5 `counter N` passes `divider N` on clocks (rings: pulse
output, 10 gt clocks; bulb chains: square, 2 gt clocks).

## Mechanisms worth knowing

* A copper bulb toggles only on a rising edge: bulb + comparator + observer is a min-pulse-1 rising
  edge detector (2 gt pulse) in 4 blocks; it is the front of every burst here.
* Two observers face to face chase each other (each fires on the other's change): a 2 gt pulse
  every 4 gt for as long as they face; a sticky piston moving one of them gates it in 3 blocks.
* An observer powering a stone fires everything around that stone at once (the square dividers'
  dual-edge front end replaces the ring's input block).
* A repeater holds one pending tick (LevelChunkTicks.schedule is a set): a second input pulse
  while its off-tick is pending is lost; observers merge edges 4 gt apart or closer.
* Don't let the judge's lamp close a loop: a lamp is a conductor; the judge now refuses a design
  whose output changes when the lamp is replaced by air.

## Tools (mctas circuit pulsemult ...)

* `search --seed <ports + fixed blocks + circuit: kind> --box ... --palette ...`: random search
  (a stone goes under every dust/diode/torch automatically; screen, then the judge). Weak past
  about 7 blocks; hand-build a big working design and shrink it instead.
* `improve <designs> [--want 'metric value' | 'metric <= N'] [--grow 1]`: local search keeping the
  axis value (N; L and class; T; P); found the 12-block burst 3 from 20, the 19-block burst 4 from
  32.
* `as --kind 'divider {n}' <designs>`: judge designs under another kind (`{n}`, `{2n}`).
* `square-divider --n 3,5,7`: the odd square dividers for any N.

## Open

* Bursts of N >= 5: a longer gate on the chase clock (about L / 4 pulses), or a bulb chain counting
  the chase's own pulses that toggles the gate bulb off (2^k pulses in about 2k blocks).
* A burst with one re-trigger class over every delay (`ignores` or `adds`): all archived are
  `mixed`.
* A divider faster than 10 gt for N not a power of two (rings stop at 10 gt).
