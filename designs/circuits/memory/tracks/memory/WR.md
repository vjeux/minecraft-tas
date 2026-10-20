# WR bar — study 5, memory (latches, D/T flip-flops, shift, counters)

What a world-record entry means here, per variation: the record target, then what we have, then the gap. "Blocks" is always `mctas circuit verify`'s count. Judged in the game or it does not exist.

| variation | record target | ours today | gap |
|---|---|---|---|
| RS latch | 4 blocks (two droppers + comparator + one item) | rs-dropper 4b | at the bar; a 3-block RS is unlikely (a latch needs a readable state block + its reader) |
| D latch, enable high | 5 blocks | d-latch-high 5b (locked repeater + subtract comparator) | at the bar |
| T flip-flop | 3 blocks (copper bulb + comparator + its stone) | tff-bulb 3b | at the bar: the bulb IS the state, the comparator is the only reader |
| T flip-flop with reset | tff + one block that empties the state | tff-reset | measure; a reset that does not grow the cell is the bar |
| D flip-flop (edge) | 5-6 blocks | dff | measure against the latch pair |
| counter 2^K | 3K blocks (K bulb stages) | bulb chain, 3K | at the bar for K <= 5 |
| counter N, N small and composite | chained rings / ring + bulbs, 8-20 blocks | counter3-corner 8b, counter9-ring3-ring3 17b | at the bar |
| **counter N, N large or prime** | **CONSTANT size, independent of N** | ring per unit: counter 19 = 54b, counter 23 = 72b, counter 25 = 72b, counter 38 = 57b, counter 39 = 46b | **OPEN, the one real gap: see "Item counter" below** |
| tileable latch / TFF slice | a 1-wide slice that tiles at pitch 1 with no gap | none archived | OPEN |

## Item counter (critique C36): the mechanism, and what the game actually allows

The idea: hold exactly N items in a container, move one out per input pulse, and read EMPTY with a comparator. A comparator on a container is 0 only when the container is empty and >= 1 for any single item (one item in a 9-slot dropper gives level 1), so "empty" is exact for any N up to 9 x 64 = 576 per dropper. Size does not grow with N, so for every large N this beats a ring, which needs a dropper and a hopper per unit of N.

Measured in the game, 2026-10-20 — the naive 5-block version does NOT work, and the reason is quasi-connectivity. Two droppers facing each other (the rs-dropper structure) with N items in A, both fired by one dust line over their lids: `counter4-items`, 6 blocks, level 1 for every one of 13 pulses, i.e. the pile never walks. Both droppers fire on the same pulse, so while both hold items they each move one into the other and nothing changes. Probes isolated the rule, which decides every layout here:

- A dropper fires when the OR of (a signal on itself) and (a signal on the block ABOVE it) rises - `DispenserBlock.neighborChanged`: `hasNeighborSignal(pos) || hasNeighborSignal(pos.above())`. So a VERTICAL dropper pair cannot be driven separately at all: the lower dropper's quasi-connectivity cell IS the upper dropper, so any power on the upper one fires the lower one too. Measured: with A above B, A holding 2 items, a dust on A's lid, B unpowered and not even reachable by wire, A still never drains - B fires with it and hands the item straight back. Remove B (A facing air) and A drains in exactly 2 pulses, which is the control.
- Therefore the pair must be HORIZONTAL (A and B side by side): then their quasi-connectivity cells are the two separate lid cells.
- A dropper that is held continuously powered sees no rising edge and does not fire. That is the cheapest possible gate: no gate blocks, just a level joined onto the dropper or onto its lid cell's neighbourhood.
- A comparator's side input suppresses it (compare mode: side >= rear gives 0), so no hold line may run beside the comparator that reads the dropper draining in that phase.

So the design needs a phase latch, and these parts, all proven separately in our archive:
1. Two droppers facing each other HORIZONTALLY, A preloaded with N items (the rs-dropper structure, 4 blocks).
2. Two comparators, one per dropper, each 0 exactly when its dropper is empty. "A empty" happens once every N pulses, "B empty" once every N pulses, half a period apart.
3. A phase latch toggled by those two empty signals, whose two outputs HOLD the dropper that must not fire.
The latch is either a copper bulb T flip-flop (bulb + comparator) toggled by "A empty OR B empty", or a cross-coupled torch pair (RS NOR) whose two torches ARE the two inverters the empty signals need anyway. The counter's output is free in both cases: the bulb's comparator (toggles once every N pulses) or either torch.

Working layout rules for the next build (from the attempts, all verified individually):
- Pulse both droppers from one dust line on their lids, or from a dust line one level higher sitting on two stones that cap the droppers (that frees the lid level for routing).
- Hold a dropper either directly (a dust or torch adjacent to it, or a strongly powered block adjacent to it) or through its quasi-connectivity cell (anything powering a neighbour of its lid cell).
- Put the two comparators on opposite sides (west of A, east of B) so neither hold line has to pass a comparator's side.
- A standing torch does not power the block it stands on; dust does not power the block above it; dust DOES strongly power the block below it; a torch strongly powers the block above it. A torch under a hold block is the cheapest inverter, since it both inverts and strongly powers upward.
- The bulb must be declared `lit=false`: the judge's placement leaves B empty, so "B empty" is already high when the design settles and the bulb sees no rising edge then.

Expected size, constant for every N: about 20 blocks with the torch latch, about 30-45 with the bulb and its routing. That is the record for every N whose ring costs more (today: N >= 19), and it covers the whole large-N range at once, since only the preloaded item count changes.

## Fold done 2026-10-20

Nine same-N counters that were strictly bigger than another entry with the same metrics line are out of the archive (`circuit archive --remove`): counter12-ring3-ring4 18b, counter16-ring4-ring4 20b, counter18-ring9-bulb1 28b, counter20-ring5-ring4 22b, counter2-ring 8b, counter4-ring9 9b, counter4-ring 10b, counter5-star 12b, counter9-ring9 25b. counter3-ring and counter3-corner are both 8 blocks, so they stay as equivalent layouts. counter 38 at 57 blocks beside counter 48 at 20 is not a mislabel: a composite N chains into small rings while a prime factor forces one long ring, so size tracks the factorisation of N, not N.

## D flip-flop under R7: where the rebuild stands (2026-10-20)

The latch is rebuilt and archived: `d-latch-r7`, 8 blocks, enable high, robust. The fix is clk-rt's rule, the same one that rebuilt the gates: an input that feeds a comparator's SIDE must arrive on the design's own repeater in the input cell, because a comparator side never reads the judge's powered stone. (d-latch-high, which had the enable arriving as a bare input cell, is removed: under R7 it reads 0 and the latch never opens.)

The flip-flop is NOT done, and the honest state after a long session on it: the two-latch shape is right, each latch works alone, and the best build so far gets the FIRST clock cycle correct and then holds — `dff-r7` in this folder, 30 blocks, failing at "M 2 clk rises: q = 1, want 0". Everything learned, so the next attempt starts from the facts and not from scratch:
- A subtract comparator takes the MAX of its two side cells (`getAlternateSignal`), so the master's enable (NOT clk) and the slave's enable (clk) must never touch the same cell. That is the whole geometric difficulty: with the latches side by side their side cells are 2 apart, and the clock must reach both while being inverted for exactly one.
- Routing the clock BELOW the latches (y = -1 or a dust lane south of them) is what finally got a clean build; carrying it above fails on support (a dust needs its own floor, and the latches' redstone blocks are already there).
- The data must ALSO arrive on its own repeater when it feeds a comparator side, not only the enable. A bare dust in the d cell reads 0, exactly as the enable did.
- Judged dead ends, each measured rather than guessed: a bulb-gated repeater (the bulb toggles per edge, so q follows the bulb and not d), an observer on the clock's input block (its 2 gt pulse is shorter than the judge's clock phase), both latches in one row (shared side cell), the clock carried on a layer above (support), the torch moved to the slave side (same failure mirrored), and the output taken from the master's repeater (then q is the master, not the flip-flop).
- Next: keep the working y=-1 clock lane, but separate the two latches along z instead of x, so the enables arrive from opposite faces and the data path never crosses the clock lane.

### The repeater-lock route (coordinator's suggestion, 2026-10-20)

A repeater's side lock reads only diodes, which the design owns, so it avoids the comparator side-MAX problem entirely. It also found the bug behind every earlier failure: **a repeater's `facing` is the direction it READS from**, so lock repeaters that "point at" a latch in the layout were pointing away from their own inputs. With that corrected the repeater-lock LATCH passes: 6 blocks, enable low, 4 gt, robust (it does not beat the archived 4-block d-latch-repeater, so it is not archived).

The master-slave built from two of them still fails, and the failure is now specific and repeatable: q is TRANSPARENT (`M 1 d=1 clk=0: q = 1, want 0`) instead of held, so at clk=0 both repeaters pass instead of only the master. Six placements of the two lock repeaters were judged (both locks north of the data line, both south, one each side, the torch on the master, the torch on the slave, and the clock entering from the far side); each either leaves q transparent or fails the support check. The slave probe on its own also fails to pass data, which says the slave's lock is permanently engaged rather than clock-driven — that is the thing to isolate next: build ONLY the slave latch, confirm it opens and closes with its enable, and only then join it to the master.

Until it passes, `dff`, `dff-enable` and `shift 2` stay empty rather than holding a design that fails the judge.

Re-judged both studies under R7 and removed what fails the judge: rs-loop (12b), dff-window (10b), dffe-window (13b), shift2-window (20b), tff-window (17b), tff-masterslave (24b). All six are window or master-slave designs that time their copy against the input's own edges; R7 drives the input with a repeater, whose shortest pulse is 2 gt and whose edge lands inside a tick rather than between ticks, so the window never opens where the design expects. Widening the window repeater (delay 2, 3, 4) does not fix dff-window: the clock edge itself is in the wrong phase, not merely too short.

Consequences, stated plainly rather than hidden:
- `dff`, `dff-enable` and `shift 2` now have NO archived design at all. They need a rebuild whose clock edge is taken from a component inside the design (a torch on the clock's input block, or an observer) rather than from the input's own timing.
- `tff` keeps tff-bulb (3 blocks) and the rest of the family, which all survived; the two removed TFFs were the fat ones (17 and 24 blocks) that WR.md already listed as first implementations.
- `rs-latch` keeps rs-dropper (4b) and rs-crafter (6b); `d-latch` keeps both; every counter except the refused dropper rings survives.

The 18 dropper-ring counters R7 REFUSES (no side of their input cell leads out of the build) are a different matter: they are not wrong, the judge simply cannot drive them. The item counter (39 blocks, any N) already replaces every large-N one, so the gap they leave is only at small composite N, where the bulb chain and the corner ring cover it.

## Sizes after the rebuild (2026-10-20)

| kind | design | blocks | note |
|---|---|---|---|
| dff | dff-lock | 9 | rising edge, latency 6 gt, setup 1 gt, hold 1 gt, robust, irreducible. Was 15: the clock block itself is the master lock's back and carries the slave's wall torch, so there is no clock wiring at all |
| shift 2 | shift2-lock | 27 | was 30: stage 1 is the 9-block dff on the clock block; a dust on the clock block carries the clock round to stage 2's stone (18 blocks for stage 2 — the loop-back from stage 1's output to a second stage sharing the clock block and torch from their other faces would cost more than it saves) |
| dff-enable | dffe-lock | 12 | was 18: the 9-block dff plus a second lock on the slave from e (wall torch + repeater + stone); latency 6 gt, robust, irreducible |

The dff went 31 -> 18 by the in-game shrink, then 18 -> 15 by hand: the shrink removes blocks but never re-routes, and the clock line it left ran three cells further than it needed. The stage is two repeaters on the data line (B west, A south locking B from the side), a wall torch for the inverted phase, and one clock dust above; a second stage reuses the same dust.

dff-enable went 31 -> 18 by NOT gating the clock at all. The first build ANDed e into the clock (31 blocks, latency 38 gt, position-dependent), and every cheaper AND failed on support: a torch cannot hold the dust above it, a stone the dust reads has no support either, and the NAND one layer lower arrives a tick late and misses the first edge. The fix is a repeater fact: a repeater is locked when EITHER side is powered (`getAlternateSignal` takes the max of both sides), so the enable needs no gate, only its own lock on the slave's free side — a wall torch on the e cell and one repeater, 3 blocks. With e off the slave stays locked through any clock edge and q holds; the master still tracks d, which is harmless because the slave only reads it on an edge with e on. The judge changes e only while the clock is low (seq.rs), the same setup assumption every enable flip-flop makes.

## ONE design per kind (vjeux 21:47Z, 2026-10-20)

- D latch: d-latch-repeater, 4 blocks (transparent while e is off). The 8-block enable-high latch is removed: the same 4 blocks with the 1-block NOT on the enable (a wall torch on the e block feeding the side repeater) is an enable-high latch in 5 blocks, judged robust (latency 6 gt), and the caption says so.
- RS latch: rs-dropper, 4 blocks, 6 gt, tileable every 2. rs-crafter (6 blocks, 13 gt) removed.
- D flip-flop 15, with enable 18, shift-2 30: one each. Open: shift-2 should beat 2 x dff by sharing the inverted clock (one torch for both slaves) once the two phases run on opposite sides of the data line.
- T flip-flop with reset: tffr-dropring 8 blocks, the only one; open.
- Counters: one per N by the fold (item counter from 17 up, clk-rt's ring counters and the bulb chains below).

## Next target: the item counter (39 blocks today, any N from 17 to 576)

Same mechanism, priced part by part from today's tricks, should land near 18:
- A, B: two droppers facing each other, the pile in A (2).
- Input: a dust lying on each dropper, the input block beside one of them (2). A HELD dropper (already powered) does not fire, so the input pulse fires only the free one.
- Hold: comparator C reads the bulb W and points into a stone X beside A (W lit: A held); a wall torch hanging on X sits beside B (W off: B held) (C + support + X + torch = 4).
- Empty detection: comparator a reads A and points into stone Ka, comparator b reads B into Kb; a wall torch on each is lit while its dropper is EMPTY (a, b, 2 supports, Ka, Kb, 2 torches = 8).
- W: the copper bulb, touching BOTH torches (the 2-block NAND trick: a bulb is powered by any neighbouring torch, and a torch also powers a bulb directly above or below it) (1).
The hard part is geometry, not logic: Ka and Kb sit on the far sides of A and B, and both torches must touch W. Torches above or below W count too, which is the freedom a layout should use. Worth a small layout search (a few roles, a 5x4x5 box) rather than hand placement.

### Item counter layout notes (2026-10-20, in progress)

Pieces that fit together (each checked against the source, none judged yet as a whole):
- Detectors staggered so their torches share a bulb: comparator a at (0,1,-1) reads A; comparator b at (1,1,-2) reads B THROUGH a solid block at (1,1,-1). Their output blocks Ka (0,1,-2) and Kb (1,1,-3) are diagonal, so torches standing on them, (0,2,-2) and (1,2,-3), both touch a bulb W at (1,2,-2). A stone at a comparator's side is ignored (getControlInputSignal reads only redstone blocks, dust and diodes).
- C at (1,2,-1) reads W and sits on the through-block (no support block). Its output faces the cell above B.
- Input: the judge's stone directly above A (powers A, updates it); a dust lying on B, next to that stone, carries the input to B (dust reads a strongly powered neighbour without connecting to it).
Traps found:
- A solid block above B, powered by C, also quasi-powers A: A checks the cell above itself, and that block is a neighbour of it.
- The inverted hold (a torch for NOT W) must not be fed by the input path: if the input dust also powers the torch's block, the torch blinks with every pulse and re-fires the held dropper when it relights (a dropper fires on every rising edge of its power).
- Pile starts in the dropper that the hold leaves free while W is off.

### Item counter: 39 -> 23 blocks (2026-10-20)

`mctas circuit memory items --n N` now writes the 23-block layout (every N from 11 up judged robust in the game; it now also beats the ring counters at N = 11, 13, 15, 21, 22, 26, 27, 28, 30, 33 and 45). What made it small:
- Detectors on two sides of the pair: comparator a reads the left dropper from the west, comparator b reads the right one from the north; b's torch hangs right above the bulb, a's reaches it through two dusts and the stone under the second dust.
- The phase comparator C points into the stone UNDER the left dropper (that holds it), and a dust under the right dropper, lit by that stone, switches off the torch standing beside the right dropper (which held it). A dust does not power the block above it, so the right dropper only sees the torch.
- Input: the judge's stone on the right dropper's lid, a dust on the left one's lid.
The rule that cost an hour: a dropper must never be STRONGLY powered — a torch directly under it, or a comparator pointing into it. A strongly powered dropper powers the dropper beside it, which then stays triggered and never fires again. Both holds are therefore weak: a stone under one, a torch beside the other.
