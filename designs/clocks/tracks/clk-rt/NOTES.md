# clk-rt notes: redstone-tick clocks (torches, repeaters, comparators, dust)

Track of the clock study (brief: `designs/clocks/CLOCKS.md`; definition and judge:
[docs/clocks.md](../../../../docs/clocks.md)). Family: every block whose state changes is a torch,
repeater, comparator or dust. Every clock below passed `mctas clock verify` (the real game, 8
positions x 4 rotations = 32 builds, `robust`) unless marked; `clock sim` (redsim) agrees on all.
Tools: `mctas clock rt <command>` (crates/clock-rt):

| command | what |
|---|---|
| `uloop --head torch\|corner\|dust2 --top 2,1* --bottom 3,d --tail dust\|torch\|fade` | one U-loop design (families.rs) |
| `sweep-uloop --kmax K [--alphabet ...] [--powered] [--per-family] --out-dir D` | every U-loop up to row length K judged by clock sim: best per period, every waveform, best per (period, family) |
| `sweep-ring --w W --h H [--straight ...] [--corner ...]` | every assignment of a W x H rectangle ring (ring.rs: corner blocks, torches, repeaters, comparators, dust) |
| `sweep-odd --kmax K` / `sweep-odd-ring --w W --h H` | odd periods: every loading of every loop with an odd half loop delay, kicked at odd ticks (odd.rs) |
| `sweep-burnout --kmax K [--max-at N]` | fast loops (torches burn out), self-started and kicked: every burnout period |
| `kick <design> [--odd]` | try temporary-block kicks (redstone block by a dust, torch under a block) at every tick |
| `divider --d1 D1 --d2 D2 --stages K` / `divider --fade C --stages K` | divider chain: base U-loop or fading ring + K master-slave T flip-flop stages |
| `shrink <design> [--period P] [--pairs]` | local search for a smaller clock (single-cell and remove-two-add-one moves) |
| `table <files or dirs>` | best design per period over existing designs (clock sim) |

Archive: clk-rt archives its own clocks (`mctas clock archive`, coordinator protocol 10-19);
generated names carry the family: `uloop<n>t-` (n torches), `fade-`, `ring<w>x<h>-`, `div-`,
`odd-`, `burn-`.

## Results (fewest blocks per period, all robust)

| P (gt) | blocks | family | design | structure |
|---|---|---|---|---|
| 4 | 8 | comparator+redstone_wire | `cmp-sub-4b.txt` | subtract comparator, rear = redstone block, output block + 2 dust into its side (15 -> 1 -> 15) |
| 5 | 18 | redstone_wire+repeater | `odd/ring-p5.txt` | ODD: 5 delay-1 repeaters + dust in a ring, one pulse loaded at placement, a second one by a 1 gt redstone block `at 3` |
| 6 | 13 | redstone_wire+repeater | `ring/3x3/ring3x3-p6-rw-r.txt` | 3x3 rectangle ring: 3 corner blocks + a dust corner, 3 delay-1 repeaters + 1 dust |
| 7 | 22 | redstone_wire+repeater | `odd/ring-p7.txt` | ODD: 7 delay-1 repeaters, two 2 gt pulses, kick `at 5` |
| 8 | 10 | redstone_wire+repeater | `sweeps/uloop-k2/uloop-p8.txt` | pulse ring, 2 delay-2 repeaters |
| 9 | 18 | redstone_wire+repeater | `odd/ring-p9.txt` | ODD: 3 delay-2 + 3 delay-1 repeaters, two 4 gt pulses, kick `at 5` |
| 10 .. 20 | 8 | redstone_torch+repeater | `uloop-p10` .. `p20` | 2-torch U-ring: torch on A -> repeater -> block F -> torch on F -> repeater -> A |
| 22, 26, 30, 34 | 12 | redstone_torch+repeater, repeater | `uloop-p22` .., `ring3x3-p22-r` .. | 2-torch ring with 4 repeaters; 3x3 ring of 4 repeaters + 4 corner blocks |
| 24, 28, 32, 36 | 9 | redstone_torch+redstone_wire+repeater | `uloop-p24` .. | 1-torch inverter: corner head, 2 repeaters, torch tail |
| 38, 42, 46, 50 | 16 | redstone_torch+repeater | `sweeps/uloop-k3/` | 2-torch ring, 6 repeaters |
| 40, 44, 48, 52 | 12 | redstone_torch+redstone_wire+repeater | `ring/3x3/ring3x3-p40-rt-rw-r.txt` .. | 3x3 ring: 3 repeaters, 3 corner blocks, a torch on the third block, a dust corner |
| 54, 58, 62, 66 | 20 | redstone_torch+repeater | `uloop-2torch/p54.txt` .. | 2-torch ring, 8 repeaters |
| 56, 60, 64, 68 | 13 | redstone_torch+redstone_wire+repeater | `uloop-p56` .. | 1-torch inverter, 4 repeaters |
| 70, 74, 78, 82 | 24 | redstone_torch+repeater | `uloop-2torch/p70.txt` .. | 2-torch ring, 10 repeaters |
| 72 .. 100 (0 mod 4) | 17 | redstone_torch+redstone_wire+repeater | `sweeps/uloop-k3/` | 1-torch inverter, 6 repeaters |
| 86, 88 | 17 | comparator+redstone_torch+redstone_wire | `fade/sweep/uloop-p86.txt`, `p88` | comparator FADING ring (below) |
| 90, 94, 98 | 28 | redstone_torch+repeater | `uloop-2torch/p90.txt` .. | 2-torch ring, 12 repeaters |
| 114 .. 338 | 18 .. 34 | comparator+redstone_torch+redstone_wire | `fade/sweep/uloop-p114.txt` .. | FADING rings: P114 18b, P142/P144 21b, P170 22b, P226 26b, P282 30b, P338 34b |
| 205 | 8 | redstone_torch+redstone_wire+repeater | `odd/burnout-odd-p205.txt` | ODD burnout: 2-torch loop (delay 6) kicked by a temporary wall torch `2 at 1`, 3 gt pulses |
| 220 | 8 | redstone_torch+repeater | `uloop-p220` | burnout of a 2-torch + 2 delay-1 ring (also 9-block 1-torch `burnout-tr9.txt`) |
| 410 | 12 | comparator+redstone_torch+redstone_wire | `ring/3x3/ring3x3-p410-c-rt-rw.txt` | burnout in a 3x3 ring (rotation-dependent) |
| 440 | 8 | redstone_torch+repeater | `burnout/burn-p440.txt` | 2-torch burnout, odd kick `1 at 15`: the two burnout halves land in different parity classes |
| 40 .. 2304 | 27 .. 117 | redstone_torch+redstone_wire+repeater | `div/div-*.txt` | DIVIDERS: base U-loop + master-slave T flip-flop stages |
| 228 .. 5408 | 36 .. 106 | comparator+redstone_torch+redstone_wire+repeater | `div/div-fade*.txt` | DIVIDERS on a fading ring: P1012 64b, P1360 76b, P2704 88b, P5408 106b (P10816 124b: sim) |

Every even period from 4 to 100 has a robust clock; odd: 5, 7, 9, 205. Every (period, family)
pair's smallest U-loop: `sweeps/uloop-k2/uloop-p<P>-<fam>.txt` (127 designs, 119 robust:
`sweeps/uloop-k2-verify.txt`; the 8 others are rotation-/position-dependent: equal-tick repeater
races); 3x3 rings: `ring/3x3/` (78, 77 robust: `ring/3x3-verify.txt`).

### Block-count curves
- 1-torch inverter U-loop with k repeaters per row: 4k + 5 blocks, P = 4 (1 + sum of delays) up to
  32k + 4: about P / 8 blocks for long periods.
- 2-torch U-ring: 4k + 4 blocks, P = 4 + 2 x sum of delays, 4k + 4 <= P <= 16k + 4 (P >= 10): the
  P = 2 mod 4 workhorse (inverter loops are always 0 mod 4).
- Pulse ring of repeaters (corner head, dust tail): 4k + 6 blocks, P = 2 x sum (one pulse); with
  corner BLOCKS (a repeater reads a block from any side): 3x3 ring of 4 repeaters at 12 blocks.
- Comparator fading ring with k comparators per row: 4k + 10 blocks, P = 56k + 2 (about 14 laps of
  4k gt): about P / 14 blocks, the best linear family.
- Dividers: + 18 blocks per doubling: 9 + 18k blocks for P = 4 (1 + d1 + d2) x 2^k on a U-loop
  base, 4c + 10 + 18k for (56c + 2) x 2^k on a fading base: exponential. A stage beats lengthening
  the loop once P > ~250 (fading) or ~150 (inverter).
  Verified: P40 (27b), P288 (63b), P1152 (99b), P2304 (117b); fading base P228 (36b), P1012 (64b),
  P1360 (76b), P2704 (88b), P5408 (106b).

## Comparator fading ring

```text
  z=-1:  Dn  c> c> .. c>  F  T     top row: compare comparators (input west) into block F
  z= 0:  D0  <c <c .. <c  D1 J     bottom row: comparators (input east) read D1 (dust beside F)
```
The comparators keep a signal strength around the ring (a comparator reading a comparator, a
block or the dust beside a block passes the strength unchanged); the turn D0 -> Dn (two dust)
loses one level per lap. The torch T on F's east face is off while F carries any strength; when
the ring is empty it lights and feeds 15 into J, so D1 = 14 and the ring refills. Period = about
14 laps + the refill. Head `C` (block + dust) instead of `D` makes the turn lossless; a dust cell
in a row then supplies the loss.

## The U-loop

```text
  z=-1:  H  t1 t2 .. tk  F      top row: the signal runs east; F a block the last top cell powers
  z= 0:  A  b1 b2 .. bk  L      bottom row: runs west into A; L the tail beside F
```
HEAD = a wall torch H on A (an inversion; the first top cell reads it directly) or a corner (A +
a dust beside it). TAIL = a dust beside F or a wall torch hanging on F (an inversion; the first
bottom cell reads it). Cells: repeaters (optionally powered at placement: loaded pulses),
comparators, dust, a block + a wall torch on it. The torches are the cheapest turns (1 block, no
floor), so the 2-torch ring (both ends torches) is the smallest loop; with an odd number of
torches it is an inverter loop.

## Divider stage (master-slave T flip-flop, 18 blocks)

```text
  z=-2:  w  w  .  .       w dust (the inverter path back to M)
  z=-1:  w  M  L1 T       M master latch (repeater), L1 a repeater reading the previous torch T
  z= 0:  T' S  L2 A       S slave latch (reads M), L2 a repeater reading the previous block A
  z= 1:  A' x  .  .       S -> dust x bending into the TARGET block A' -> torch T' on A' -> dust -> M
```
L1/L2 point into the latches' sides: M is locked while the previous stage's S is low, S while it
is high, never both unlocked: the loop M -> S -> A' -> T' -> M (one inversion) moves one step per
half period of the previous stage, so T' toggles once per previous period. The output pair (T'
north of A') has the input pair's orientation: the next stage is the same layout 3 west, 1 south.
A target block (a conductor dust connects to) turns the slave's dust into A' (a plain block would
leave the dust a straight line pointing past it).

## Theory

**Every delay is even.** torch 2, comparator 2, repeater 2d, burnout re-check 160, dust 0. A
scheduled tick runs at (scheduling time + even); a neighbor update runs in the tick of its cause.
So every state change of a clock that was only placed (the judge places all blocks between two
ticks) happens at placement + even: one PARITY CLASS; pending ticks keep their parity, so the
period is EVEN. A kick at an even offset keeps one class.

**P = 1, 2, 3 are impossible.** A torch, repeater or comparator changes state only in its own
scheduled tick, scheduled >= 2 gt ahead, and a position holds at most one pending tick of its block
(`LevelChunkTicks.schedule` refuses a second (pos, block) pair): two changes of one component
are >= 2 gt apart. Dust follows the components inside the tick. A tick that changes nothing
schedules nothing new.

**Odd P needs two parity classes**: events started at placement (even) and a kick at an ODD tick,
mirroring each other: shifting time by P maps every even-class event onto an odd-class one.
- Inverter loops can't: their pattern satisfies f(x + D) = not f(x) with D even, and an odd period
  g = gcd(P, D) divides D.
- Non-inverting rings can: two identical pulses P apart in a ring of delay 2P (P5: 5 delay-1
  repeaters, pulses of 2 gt; widths and gaps must stay >= every repeater's delay, strictly when a
  repeater feeds a repeater: the upstream one's EXTREMELY_HIGH tick runs first).
- Burnout clocks can (P205): the second class enters through the torch memory's timing.
- Flat rings of repeaters + corner cells hold an even number of cells (a rectilinear loop has an
  even number of corners and the cell cycle has even length): an odd count needs a dust cell.

**Torch burnout** (`RedstoneTorchBlock`, global `RECENT_TOGGLES` list per level): only turn-offs
are logged; the 8th turn-off within 60 gt burns the torch out and schedules a re-check 160 gt
later; the re-check relights it if its input is off (entries older than 60 gt are purged at the
start of every torch tick). The re-check is DROPPED if a relight tick is already pending (one tick
per position): a torch whose input falls the moment it turns off (a torch + dust self-loop, P4)
schedules its normal relight first, that relight finds it burnt out, and it stays dark for good.
With a delay in the loop the input is still on at the 8th turn-off, the 160 gt re-check wins, and
the input's later fall is the one dropped.
- A torch in the active set turns off at least once per period: P <= 8 burns it out, so torch
  families cannot reach P 4-8 (`designs/clocks/impossible.tsv`); inverter loops have
  P = 2 x (loop delay) = 0 mod 4, so 1-torch loops start at 12; a P8 torch loop is a 220 gt
  burnout clock: 4 + 7 x 8 + 160.
- A 2-torch ring with loop delay 8 burns both torches: block states repeat every 220 gt, the
  torch memory every 440 (`clock sim` state period 440, visible period 220).

**Pulse rings in practice** (from the sweeps): a repeater passes a pulse only if it is at least
its delay long and the gap after it too; a repeater fed directly by a repeater needs the gap
STRICTLY longer (the feeding repeater's EXTREMELY_HIGH tick runs before the fed one's VERY_HIGH
turn-off in the same tick). Comparators pass pulses >= 2 gt and swallow 1 gt ones.
