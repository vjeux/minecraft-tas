# Study 12: computational redstone: variations

Binary arithmetic and routing circuits: adders, subtractors and incrementers, comparators,
multiplexers, demultiplexers, decoders and encoders, and binary counters with a parallel output.
This page lists what can vary between designs, read in the 26.3 source (`mctas src show <Class>`,
no outside knowledge) and in our own archives, and says how the judge measures each variation.
The judge kinds come next (clockinfra reviews), then the best design per variation.

## 0. Scope, and links to the other studies

* Study 4 (gates) has the 2-input functions. A half adder is XOR (sum) + AND (carry), so its
  archive gives this study's building blocks and their measured cost: the instant XOR
  `gate-xor-ring-14b` (a ring of dust, 0 gt), AND by comparator (4 gt, 5 blocks), OR by bare dust
  (0 gt), NOT / NOR / NIMPLY by torch (2 gt). A 2-input function judged here would duplicate study 4,
  so this study starts at 3 inputs (full adder, 2:1 multiplexer).
* Study 5's `counter N` is a mod-N pulse counter with ONE output (it toggles, pulses or repeats a
  level every N pulses). A binary counter here has N outputs that read the count in binary
  (q0 = the low bit) after every pulse. Study 5's `counter 2^N` needs the top bit only; the binary
  counter must also show the lower bits. Study 2's T flip-flops are the classic ripple-counter
  stage, so a binary-counter bit slice is a T flip-flop plus a carry.

## 1. Components and their delays (26.3 source)

Latency is what most variations trade against size, and it is set by these parts:

| part | delay | source facts that matter |
|---|---|---|
| redstone dust | 0 gt | power = the strongest neighbor - 1 (a source gives 15), recomputed within the tick (`RedstoneWireBlock` + `DefaultRedstoneWireEvaluator`); a dust's shape (where it points) also routes power, which is how the instant XOR works |
| redstone torch | 2 gt | inverts its support block's power after a scheduled tick of 2; BURNS OUT after 8 toggles within 60 gt (`RECENT_TOGGLE_TIMER` 60, `MAX_RECENT_TOGGLES` 8) and stays off for 160 gt |
| repeater | 2..8 gt | delay 1..4 redstone ticks; outputs 15; LOCKS while a powered diode faces its side (`sideInputDiodesOnly`) |
| comparator | 2 gt | compare mode: back if back >= side, else 0; subtract mode: back - side (`calculateOutputSignal`); its back reads a container or analog block directly, or through one solid block; its side reads dust, redstone blocks, diodes and other signal sources (`getControlInputSignal`) |
| observer | 2 gt | 2 gt pulse on any change of the block it faces |
| copper bulb | 0 gt | toggles on a rising power edge inside `neighborChanged` (no tick); a comparator reads 15 while it is lit |
| piston | block events | moves in the block-event phase (study 10: a QC piston's push begins the tick after the update and lands 2 gt later) |

Container fill to comparator strength (`AbstractContainerMenu.getRedstoneSignalFromContainer`,
`Mth.lerpDiscrete`): f = the mean fill of the slots (count / max stack); strength =
floor(14 f) + (f > 0 ? 1 : 0).

## 2. Encodings

* **Binary lines** (the default): one cell per bit. The judge drives an input bit the usual way (a
  redstone block placed with updates for 1, air for 0) and reads an output bit as the level into
  its cell (on = level > 0, with the judge's lamp there: study 4's update-correctness rule holds).
* **Signal strength** (hex): a 4-bit value 0..15 on ONE line. Input: `port input.a: x y z
  strength` on a single chest of the design (redsim models chests, not barrels; single chests never
  join, since `ChestBlock.updateShape` only joins a chest that already points at its neighbor): the
  judge rewrites its contents to the item count whose strength is the value (n items of a 64-stack:
  the smallest n with floor(14 n / 1728) + 1 = v), and the design reads it with its own comparator
  (counted). Output: the level read in the output cell (a dust's power or a comparator's strength),
  which must EQUAL the value, not just be on. Calibrated: `cal-hex-gt`, below.
  Variations: `hex` adders (modulo 16 or saturating), and hex comparators: a comparator in
  SUBTRACT mode with a on its back and b on its side outputs a - b when a > b and 0 otherwise, so
  it is exactly `gt` (b reaches the side through b's reader comparator, since a side input reads
  dust, redstone blocks and diodes, not containers: 2 blocks in all); compare mode is `ge` except
  at a = b = 0 (a comparator with no back input is off).
* **One-hot**: decoders and demultiplexers output it; encoders take it.

## 3. The kinds (names, ports, what must hold)

Bit k of a value `a` is the port `input.a<k>` / `output.s<k>` (k = 0: the lowest bit). Unsigned
arithmetic.

| kind | inputs | outputs | must hold for every input state |
|---|---|---|---|
| `adder N` | a0..a(N-1), b0..b(N-1), optional cin | s0..s(N-1), optional cout | s + 2^N cout = a + b + cin (no cout: s = (a + b + cin) mod 2^N) |
| `subtractor N` | a.., b.., optional bin | d0.., optional bout | d - 2^N bout = a - b - bin |
| `incrementer N` | a.., optional cin (else +1) | s.., optional cout | s + 2^N cout = a + cin |
| `comparator N` | a.., b.. | any of eq, lt, gt, le, ge, ne | eq = (a = b), lt = (a < b), ... |
| `mux N` | s0..s(N-1), d0..d(2^N - 1) | q | q = d[s] |
| `demux N` | s.., d | q0..q(2^N - 1) | q[s] = d, every other q = 0 |
| `decoder N` | a.., optional e (enable) | q0..q(2^N - 1) | q[a] = 1 (and e), every other q = 0 |
| `encoder N` | d0..d(2^N - 1) | q0..q(N-1), optional valid | one-hot d: q = the index; all zero: valid = 0 (q free) |
| `encoder N priority` | as above | as above | for EVERY input: q = the highest set index |
| `binary-counter N` | clk, optional reset | q0..q(N-1) | after k clk pulses, q = k mod 2^N; reset clears it |

N = 1 is excluded where the kind is a study 4 gate or a wire: `comparator 1` (XNOR, XOR...),
`decoder 1` (a and NOT a), `encoder 1` (q = d1). `adder 1` without cin (a half adder: two outputs)
and `mux 1` (2:1, three inputs) stay. The study's axis is the kind and N.

## 4. How the judge tests and measures

**Exhaustive truth table** (combinational kinds, n inputs):

* every input state in a Gray-code order (one input changes per step, all 2^n states);
* every single-input transition in both directions (an Euler tour of the n-cube, n 2^n changes)
  for n <= 10;
* jumps where every input changes at once: each state reached from its complement (2^n jumps:
  parallel loading, the case a ripple circuit handles worst);
* each state held for the gap (redsim's settle time + margin, at least 10 gt so that no torch can
  reach 8 toggles in 60 gt from the walk alone);
* for n > 10 (an 8-bit adder has 17), the Gray code and the complement jumps only; above 16
  inputs, bit slices (below).

Pass: in every hold, every output settles to the expected value and keeps it until the next
change. The output must also drive the judge's lamp (study 4's update-less rule).

**Metrics:**

* `latency`: the worst gt, over every tested transition, from the input change to the frame where
  every output holds its final value; the transition named (`latency 14 gt (b0 rises at
  a=0111 b=0000)`). 0 = `instant`.
* `glitches`: transitions where an output changes more than once before it settles (count;
  0 = glitch-free).
* `blocks`, `per bit` (blocks / N), box, `height` (flat = 2: one layer of blocks and one of
  components on top); `slime-free`, `piston-free`, `torch-free` from the block list.

**Bit slices** (tileable per bit), for ripple designs: the design is ONE bit with `slice: dx dy
dz` (the step to the next bit) and chain ports `port chain.in` and `port chain.out` (carry, borrow,
the comparator's running result, a counter's toggle), with chain.out = chain.in + step (bit k's
chain output is bit k+1's chain input cell). The judge builds N copies (N = 1..4 judged
exhaustively, and 8 on the complement jumps and the carry-chain worst cases), names copy k's ports
a<k>, b<k>, s<k>, drives copy 0's chain.in as cin (or leaves it air when the kind has no cin),
reads the last copy's chain.out as cout, and judges the N-bit kind. Reported: `per bit` blocks,
the latency law `latency = A + B N` from the judged N, `tileable`. A slice-built design is one
archive card for every N (a study 1 style extendable card).

**Sequential (`binary-counter N`):** 2^N + 3 clk pulses of 4 gt (spaced by the gap), the outputs
read at the end of each gap must count 0, 1, 2, ... mod 2^N; with a reset input, resets at three
different counts. Metrics: `latency` (pulse edge to the last output settled, the worst over the
counts: a ripple counter's worst is the all-ones rollover), `edge` (rising or falling, as for dff),
`min-period` (the shortest pulse spacing at which it still counts: pulse trains at 20, 10, 8, 6,
4 gt).

## 5. Variations to build (the best design of each, no near-duplicates)

| kind | variation | how it is judged | the claim to beat |
|---|---|---|---|
| full adder (`adder 1` + cin) | smallest | blocks | half adder pairs from study 4 |
| full adder | fastest | worst latency | |
| full adder | instant sum (0 gt on s) | per-output latency | the instant XOR ring |
| full adder | flat, slime-free | height, block list | |
| ripple adder slice | smallest per bit; fastest carry per bit | per bit blocks; latency law B | |
| `adder 4` | carry-lookahead (no carry ripple) | worst latency vs the slice law at N = 4 | |
| `adder 4` | instant carry (0 gt carry chain) | latency 0 on cout | |
| `adder 4 hex` | strength-encoded operands | level-exact output | |
| `subtractor N`, `incrementer N` | smallest; fastest; slice | as for adders | an adder with inverted b / b = 0 |
| `comparator 4` eq | smallest; fastest | | XNOR per bit + AND |
| `comparator 4` lt / gt | ripple slice from the MSB; fastest | | |
| hex comparator (`gt` on strengths) | a subtract-mode comparator + b's reader | level inputs, all 16 x 16 | |
| `mux 1` (2:1) | smallest; fastest; instant | | repeater lock or comparator subtract |
| `mux 2` (4:1), `demux 1/2`, `decoder 2/3`, `encoder 2/3` (+ priority) | smallest; fastest; instant | | |
| `binary-counter N` | ripple (T flip-flop chain: copper bulbs + observers, study 2) | slice, latency law | study 5 counters (one output) |
| `binary-counter N` | synchronous (every bit settles together) | worst latency | |
| `binary-counter N` | up/down, with reset | | |

Families: ripple slices are extendable cards (N copies); designs that differ only by repeater
delays are tunable groups (the catalog folds them, study 1 rules).
