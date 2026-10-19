# clk-rt notes: redstone-tick clocks (torches, repeaters, comparators, dust)

Track of the clock study (brief: `~/agents/CLOCKS.md` on the boxes; definition and judge:
`docs/clocks.md` once clockinfra lands it). Family: every block whose state changes is a torch,
repeater, comparator or dust. Measured = real game (`mctas door drive <file>.drive`), each
design has its drive script next to it.

## Measured clocks

| P (gt) | blocks | design | waveform (output) | how it works |
|---|---|---|---|---|
| 4 | 9 | `cmp-sub-4.txt` | dust 13/0, 2 on 2 off | subtract comparator, rear = redstone block (15), output -> 3 dust -> its side: out 15, side 13 -> out 2, side 0 -> out 15 |
| 4 | 8 | `cmp-sub-4b.txt` | dust 14/0, 2 on 2 off | same, the output strongly powers a block A; the dust beside A carries A's strength (15 or 1) to the side |
| 12 | 9 | `tr-9-12.txt` | torch 6 on 6 off | wall torch T on A; a delay-2 repeater reads T directly and powers F; dust beside F -> dust pointing into A. One inversion, loop delay 2 + 4 = 6, P = 2 x 6 |
| 220 | 10 | `burnout-tr1.txt` | torch: 8 pulses (4 on 4 off), then off 160 gt | BURNOUT: torch + delay-1 repeater loop, inner period 8 |
| 220 | 9 | `burnout-tr9.txt` | same | `tr-9-12` with a delay-1 repeater |

## Theory

**Every delay is even.** torch 2, comparator 2, repeater 2d, burnout re-check 160, dust 0. A
scheduled tick runs at (scheduling time + even); a neighbor update runs in the tick of its cause.
So every state change of a clock that was only placed (the judge places all blocks between two
ticks) happens at placement + even: one PARITY CLASS. The judge's state includes pending ticks
relative to now, whose parities are fixed by the class, so a clock with one class has an EVEN
period. A kick at an even offset keeps one class.

**P = 2 is impossible, and so are P = 1 and 3.** A torch, repeater or comparator changes state only
in its own scheduled tick, and a position holds at most one pending tick of its block
(`LevelChunkTicks.schedule` refuses a second (pos, block) pair): two state changes of one
component are at least 2 gt apart, so a component that changes needs >= 4 gt per on/off cycle,
and P = 1, 2, 3 would need two changes 1 gt apart. Dust changes only when a component changes
(it follows them in the same tick), so it cannot change twice per 2 gt either without one of them
doing so. Pending ticks alone cannot keep a clock alive: a tick that changes nothing schedules
nothing (only a compare-mode comparator updates its front, which schedules nothing new without a
state mismatch).

**Odd P needs two parity classes**: events started at placement (even) and a kick at an ODD
offset (`port start: ... at N`, N odd), mirroring each other: shifting time by P must map every
even-class event onto an odd-class one. Candidates: non-inverting rings holding two identical
pulses P apart (inverter rings can't: their pattern satisfies f(x + D) = not f(x) with D even, and
an odd period g = gcd(P, D) divides D). P = 5 would be a ring of delay 10 with pulses of 2 (gap 3).
Flat rings of repeaters + corner cells have an even number of repeaters (a rectilinear loop has
an even number of corners and the cycle of cells has even length); dust stepping down a level
links two cells of the same color and breaks that.

**Torch burnout** (`RedstoneTorchBlock`, global `RECENT_TOGGLES` list per level): only turn-offs
are logged; the 8th turn-off within 60 gt (t8 - t1 <= 60) burns the torch out and schedules a
re-check 160 gt later; the re-check relights it if its input is off by then (entries older than
60 gt are purged at the start of every torch tick). The re-check is DROPPED if a relight tick is
already pending (one tick per position): a torch whose input falls the moment it turns off (a
torch + dust self-loop, P 4) schedules its normal relight first, that relight finds it burnt out,
and it stays dark for good. With a delay in the loop the input is still on at the 8th turn-off,
the 160 gt re-check wins, and the input's later fall is the one dropped.
- A torch that turns off once per period needs 7P > 60, P >= 10. Inverter loops have
  P = 2 x (loop delay) = 0 mod 4, so torch loops start at 12; a P = 8 torch loop (torch + one
  2 gt element) is a 220 gt BURNOUT clock: 4 + 7 x 8 + 160.

**Families** (P in gt, D = loop delay, d = repeater delay setting):
- Inverter loops (odd number of torches / comparator side inputs): P = 2D, 50 % duty. Harmonic
  modes P = 2D / (2k + 1) when every edge spacing survives the elements.
- Asymmetric inverter loops: two paths of delays a < b merged by dust (OR) in the loop: rising
  edges take a, falling edges b: P = 4 + a + b (reaches 2 mod 4).
- Pulse rings (non-inverting loops of repeaters / comparators, one pre-loaded `powered=true`
  repeater): P = D (any even P >= 4), k pulses: P = D / k; widths and gaps >= every repeater's
  delay.
- Comparator analog loops: subtract self-loop P = 4; fading loops (strength decays 1 per lap,
  a torch refills at 0).
- Burnout clocks: 220 for a torch + 2 gt loop; others with other burst shapes.
- Long periods: dividers (T flip-flops) and counters (comparator strength).
