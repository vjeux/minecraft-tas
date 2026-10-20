# Study 11: pulse multipliers, frequency dividers, monostables: the variations

From vjeux's queue (2026-10-20), no outside lookup: the behaviour classes below come from what
builds need and from the 26.3 code paths the designs run on. What the earlier studies already
judge is linked, not repeated; each new kind lists how its judge measures it.

## Already covered (linked, not repeated)

| function | where | why it is not a new kind here |
|---|---|---|
| one fixed-length pulse per rising / falling / both edges | study 1, `rising-edge` / `falling-edge` / `dual-edge` L ([pulse](../pulse/CATALOG.md)) | a non-retriggerable monostable fed isolated pulses IS a rising-edge detector of length L; only its behaviour under a second edge is new (below) |
| maximum / minimum output length | study 1, `pulse-limiter` / `pulse-extender` L | |
| x2 frequency multiplier | study 1, `dual-edge` | one pulse per input edge: on a square clock its pulses are evenly spaced, 2 per input period |
| divide by 2 | study 2, `tff` (with `min-period`) | a T flip-flop is the /2 divider of a clock, its fastest train measured |
| /N of slow pulses | study 5, `counter N` ([memory](../memory/CATALOG.md)) | counts isolated 4 gt pulses (toggles every N, pulses every N, or a level of period N); the divider below is judged on a running CLOCK instead (duty cycle, fastest input) |
| self-running clocks | the clock study ([clocks](../../clocks/CATALOG.md)) | no enable input; the gated pulse train below starts and stops on an input |

## New kinds

| kind | variation | builds that need it | what the code says | judge measure (pass rule) |
|---|---|---|---|---|
| `burst N` (pulse multiplier) | the N axis: one input pulse -> exactly N output pulses | dispensers firing N shots (cannons, N TNT), droppers moving exactly N items, pistons pushing N times, note-block repeats | | input pulses of 1, 2, 3, 4, 6, 10, 20, 40 gt, each followed by the gap: every input from the min-pulse up gives exactly N output pulses, the output off at the end of every gap |
| | output pulse length and period | the consumer decides: a dispenser or dropper acts once per rising edge and edges closer than 4 gt merge into one action; a sticky piston aborts an extension on a too-short pulse | `DispenserBlock.neighborChanged` schedules a 4 gt tick and a position holds one pending tick of a type (`LevelChunkTicks.schedule` is a set); `PistonBaseBlock.checkIfExtend` event 2 | metrics `pulse` (output length, min-max) and `period` (rising-edge spacing, min-max): one value each when evenly spaced |
| | input-length independence | a button or a plate gives the burst, a lever held gives it once | | `min-pulse`: the shortest input from which every longer one gives exactly N |
| | re-trigger behaviour | a second press during the burst: ignored (safe), restarts, or adds N more (a queue) | | a second 4 gt input inside the span (several delays): `retrigger ignores / restarts / adds / breaks` |
| | repeat rate = xN frequency multiplier | a clock feeding it gives N pulses per input period | | `repeat`: the fastest input period (4 gt pulses) that still gives exactly N per input pulse |
| | evenly spaced xN for a given input period | a smooth multiplied clock | a burst can only space its pulses for a period it is built for | the burst `period` with a `clock: P` header: P / N |
| `divider N` (N >= 3; 2 = tff) | the N axis | slowing a fast clock (farms, timers), sequencing | copper bulbs toggle only on a rising edge (`CopperBulbBlock`): bulbs chain into /2^k | the judge drives the input with square clocks of periods 40, 30, 20, 16, 12, 10, 8, 6, 5, 4, 3, 2 gt, each for N warm-up + 4N periods: the output must repeat with period exactly N x P (one rise and one fall per N input periods) |
| | duty cycle, square vs pulse output | a square output feeds the next divider or a clock-driven machine evenly; a pulse output triggers | | `duty` (on time / period at the slowest clock), `square` when it is 50% (to the gt), else `pulse` |
| | odd N with a 50% duty | a symmetric /3, /5 | needs both input edges (a rising-edge counter can only give N-1 : 1 or similar) | `square` with N odd |
| | fastest input clock | dividing fast observer / piston clocks | redstone torches burn out at 8 toggles in 60 gt (`RedstoneTorchBlock`: `MAX_RECENT_TOGGLES` 8, `RECENT_TOGGLE_TIMER` 60, off for 160 gt): torch dividers have a frequency floor | `min-period`: the shortest P from which every slower clock divides correctly |
| | latency, fixed phase | aligning the output with the input clock | | `latency`: input rising edge -> output rising edge (constant over every output period) |
| `monostable L` | non-retriggerable | "fire once per press, ignore presses while busy": door sequences, one shot per button however mashed | | input A: isolated pulses of 1..40 gt: one output pulse of length L per input from the min-pulse up; input B: pairs of 2 gt pulses whose second rising edge comes d gt after the first (d = 2, 4, L/2, L - 2, L, L + 2, L + 6, 2L): `non-retriggerable` = one pulse of exactly L while d is inside it, a second full pulse once d is past `re-arm` |
| | retriggerable | "stay on while triggers keep coming": hold a door open while players keep walking through, a light that stays on with activity; inverted, a missing-pulse alarm (a clock stopped) | | same inputs: `retriggerable` = one pulse ending L after the LAST rising edge while d < L |
| | the L axis, long L, tunable L | seconds to minutes | | L (axis), `latency`, `min-pulse`, `re-arm` |
| `off-delay L` (level-retriggered) | output on while the input is on and for L after it falls | lights / doors that stay open L after the last plate press; debounced plates | study 1's judge rejects it as an extender on purpose (a +L stretcher) | isolated pulses of 1..40 gt: each output pulse is the input length + L exactly (on at a fixed latency); 3-pulse trains with gaps < L: one continuous pulse ending L after the last fall. Metrics `L`, `latency`, `min-pulse` |
| `on-delay T` (pulse filter) | output only once the input has been on for T, off when it falls | debouncing, "hold the button 2 s", rejecting 1-2 gt glitches (observer pulses, flicker) | study 1's judge rejects it as a limiter on purpose (a shortening delay) | isolated pulses of 1..80 gt: no output for inputs shorter than T, one pulse of the input length - T (+ a constant) for longer ones, at a fixed latency from the input's fall. Metrics `T`, `latency` |
| `pulse-train P` (gated clock) | pulses every P gt while the input is on | rapid fire while a lever is on (dispensers, item droppers), farms switched by a lever | the clock study has no enable | input windows of 20, 60, 200 gt: pulses every P gt while on (the first at a fixed latency, same phase each window), none later than `stop` gt after the input falls, output off at rest. Metrics `P`, `pulse`, `latency`, `stop` |

## For every kind

smallest (blocks, then box), fastest (latency, then span), flat (2 high), tileable (copies side by side,
as study 9's `tile`), a tunable family (N or L set by a repeater delay, a comparator mode or a
line length: one card, study 1's tunable groups), slime-free. The best design of each variation is
archived; a design bigger and no better on the variation's metric stays out.

## Results

Owners: burst, divider, pulse-train: fly (explore/pulsemult); monostable, off-delay, on-delay:
clk-rt (explore/timer). Every line below is judged in the real game (16 builds) unless it says
redsim.

| kind / variation | status | design / measure |
|---|---|---|
| divider N, pulse output | COVERED by study 5: every archived `counter N` passes `divider N` on clocks (`mctas circuit pulsemult as --kind 'divider {n}' <counters>`, redsim, N = 3..48): ring counters give a pulse (duty 3..33%) and follow clocks down to 10 gt (12 gt for rings of 19+); bulb chains (N = 2^k) are square and follow 2 gt clocks; a ring + bulb counter that toggles every N (`-div`) is a square /2N, 10 gt | linked, not re-archived |
| divider, odd N with a 50% duty | DONE: archive/pm-div3-square (13 blocks), pm-div5-square (16), pm-div7-square (23); any N with `mctas circuit pulsemult square-divider --n N` (redsim: 9, 11, 13 pass, 30 / 34 / 41 blocks). A ring + bulb counter toggles every N pulses; fed on BOTH input edges (an observer powering the input cell's stone) it toggles every N/2 input periods | min-period 20 gt: the ring's droppers take one action per edge, edges 10 gt apart |
| divider, fastest for N not a power of 2 | open: rings stop at 10 gt clocks | redsim search running |
| pulse-train (gated clock) | DONE: smallest and fastest archive/pm-train-observers (3 blocks, P 4 gt: the input's sticky piston pushes an observer face to face with another, the two chase each other until it is pulled back; latency 3, stop 5); tunable P archive/pm-train-comparator-d1..d4 (12 blocks, one tunable group, P = 8, 12, 16, 20 gt: P = 4 + 4 x the repeater's delay): the input is a subtract comparator's rear, its output returns to its side through dust and a repeater; first pulse 2 gt after the input, stops within 2 gt of the release | the torch loop (cal-torch-pulse-train, 15 blocks, P 12) is bigger |
| burst N, the N axis | DONE for N = 2, 3, 4: archive/pm-burst2-bulb (7 blocks: bulb, comparator, observer, delay-3 repeater, observer), pm-burst3-bulb (12), pm-burst4-taps (19); all min-pulse 1 (any input length), 2 gt pulses about 6 gt apart, repeat 12 / 16 / 20 gt | larger N: taps into the output cell run out of sides; the scalable route is the gated clock above with an exact gate of 8N - 2 gt from a non-retriggerable monostable (a bulb one-shot gate latches on a second press: refused) |
| burst, re-trigger class | measured on every burst: all three archived are `mixed` (a second press within ~8 gt is swallowed, a later one adds a second burst); a consistent `ignores` or `adds` burst is open (local search with `--want 'retrigger ignores'` found none from the 7-block burst) | |
