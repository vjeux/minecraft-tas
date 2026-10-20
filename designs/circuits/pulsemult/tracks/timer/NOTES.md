# Track timer (study 11: monostable, off-delay, on-delay)

clk-rt, branch explore/timer. Study 11's variations are in [VARIATIONS.md](../../VARIATIONS.md);
the judge is `crates/circuit/src/pulsemult.rs` (the three timer checkers are this track's), the
tools `mctas circuit timer ...` (`crates/circuit-timer`). Everything archived is judged in the real
game (16 builds, robust).

## What is archived

| kind | designs | how |
|---|---|---|
| off-delay, L 4-28 | off-delay-ring-L4 .. L28 (13-21 blocks, latency 0) | two comparators in a ring (C1 reads stone I, powers stone A; C2 reads A, powers stone T; a dust line T -> I); the input holds a line dust up; the ring loses `dust - 1` per round of 4 gt once the input falls. `timer rings` enumerates every line / output cell |
| monostable retriggerable, L 12-36 | mono-retrig-ring-L12 .. L36 (21-26 blocks, min-pulse 1) | a copper bulb (toggled by each rising edge), a comparator K reading it, observer O1 under K firing down into a stone whose cross dust powers I, observer O2 watching O1 firing the next 2 gt into a dust on top of I: 4 gt of strength 15 per rising edge into the same ring |
| monostable non-retriggerable, L 14-38 | mono-nonretrig-ring-L14 .. L38 (25-30 blocks, flat) | the bulb edge front (bulb, K, O1, a delay-3 repeater R) through a subtract comparator Cg into I; a dust beside A and a repeater Rb raise Cg's side while the ring is on, so every edge is ignored until it dies |
| on-delay, T 8 / 10 / 12 | on-delay-piston-gate-T8 .. T12 (10 blocks, latency 0) | the input's dust points into an empty cell X beside the output; a repeater (delay 2-4) and two dust power a sticky piston that pushes a stone into X: lit only while the stone is there AND the input still on |
| long L | off-delay-2stage (off L = 52 + 56n, n 1..7), mono-retrig-2stage (L = 60 + 56n, n 1..8), mono-nonretrig-2stage (L = 62 + 56n, n 1..8): up to about 510 gt | stage 1 = the ring above (round 4 gt: any charge fills it; its output lasts >= 28 gt) refills, through a repeater (always 15), a stage-2 ring of 2n + 1 comparators (round 4n + 2 gt, losing 1 per round: its last comparator outputs into a dust, the next dust points into I2) |

Study 1's rising-edge designs that pass as non-retriggerable monostables (L 6-9) are linked from
VARIATIONS.md, not archived again (coordinator: no near-duplicates).

## What the mechanisms taught (read before designing timers)

- A comparator ring is a delay loop: an input shorter than the round circulates as pulses instead
  of filling it. The two-comparator ring (round 4 gt) is the longest round a 4 gt charge fills; its
  loss is at least 2 per round (with stones at both ends of the return line, `dust - 1`; a single
  dust cannot close a two-comparator ring: parity), so L <= ~36. Longer rings need a filling stage.
- Diode `facing=X` = INPUT from side X.
- A lone 2 gt observer pulse never charges a comparator: the observer schedules its off tick
  BEFORE it updates its neighbours, so the comparator's tick (both NORMAL priority) runs after it
  and reads 0. A comparator whose front is a diode is prioritized (HIGH) and does catch it, but the
  next comparator then misses its 2 gt output. Charge with >= 3 gt.
- A repeater stretcher swallows a second pulse that ends before its pending off tick (delay-2
  repeater + observer pulses 5 gt apart: the second is lost; 6 gt apart: merged). A 2-observer
  chain (O2 watches O1) gives 4 gt with no stretcher.
- Two charge paths at different strengths charge the ring to whichever level C1's tick phase
  catches, so L changes with the second edge's delay ("other" at d = 5). Both paths must deliver
  the same strength: O1 into I through a stone + cross dust (15), O2 into a dust ON TOP of I (15).
  In the plane it cannot be done with plain observers (O2's back is always 2 cells from O1).
- A non-retriggerable gate must block the charge from the ring's own level fast enough, or a
  merged second edge stretches the charge: a delay-3 repeater (6 gt) before the gate makes every
  charge end when the ring blocks it, whatever the next edge does.
- A ring refilled through a dust at the previous stage's decaying level is refilled unevenly; a
  repeater (15 whatever the level) makes stage 2 exact. A stage-2 round longer than stage 1's
  shortest output flickers as stale slots die (the n limit of each family).
- A sticky piston fed a pulse <= 2 gt spits its block (the delay-1 piston gate fails on a 2 gt
  input); a delay-2 repeater in front makes every piston pulse >= 4 gt.

## Tools (crates/circuit-timer)

- `timer as <kind> <designs>`: other studies' designs judged as a study-11 kind on redsim.
- `timer rings --front input|retrig|nonretrig`: every two-comparator ring (return line, A/T as
  dust or stone, input and output cells) with that charging front, judged; the smallest per L.
- `timer long --k N [--front ...] [--file F]`: the two-stage families (the `extend generator:`).
- `timer cells <design> --stimulus '50+ 54-'`: every cell that changes, frame by frame (redsim).

## Open

- Minutes: a gated clock decrementing a lossless counter ring (three comparators, loss 0; one in
  subtract mode fed a strength-1 pulse by a comparator reading the clock's stone and a dust one
  step away). Sketched, not built.
- Flat retriggerable (the 2-observer front needs the dust on top of I), small-T on-delay (T 2-6),
  odd-L off-delays (rings give multiples of 4).
- Tileable: the `tile:` header now judges study-11 copies (each against its own input, shifted
  7 gt); the off-delay ring tiles every 5 blocks.
