# Track fresh-d: symmetric control on a SLIME-FREE mechanism

Direction (coordinator): a new door family, fc2's symmetric control (fresh-c) on a different
mechanism. Chosen: no slime at all. Every door block is pulled by its own pulse piston
(pull on the open pulse, push + spit on the close pulse), T is moved aside by its own pulse piston
P_T, and the center runs fc2's core (A held extended by a redstone block that the Q shuttle
moves away, p0, the B window that the re-extended A blocks on close).

## Mechanism (21 blocks, slime-free)
* door 9; left column: 3 sticky pullers (-2,0..2,0) facing east (gaps (-1,y,0));
  right column: 3 sticky pullers (4,0..2,0) facing west (gaps (3,y,0)); P_BM (1,-2,0) up.
* A (1,4,0) extended (head on T), RB (1,4,1) holds it; Q (0,4,1) pushes RB east to (2,4,1).
* B (1,5,0) (the UP dance: B pushes A down, A pulls C, B lifts A, A pulls C).
* P_T (3,3,0) facing west, gap (2,3,0): pulls T from (1,3,0) to (2,3,0) on open, pushes it
  back (spit) on close.

## Ideal-input study (`door ideal sf.sched` / `sf2.sched`)
* `sf-mech.txt` (P_T in -z at (1,3,-2)): Q at q=4 on BOTH edges, P_T pulse t in 5..6 works;
  with an asymmetric Q (q2 > q) t up to 7..8.
* `sf-mech2.txt` (P_T east at (3,3,0)): ONE pulse into (4,3,0) fires P_T + the top-right
  puller + the middle-right one (quasi-connectivity), t in q+1..q+2; the side pullers and P_BM
  take their pulse at ANY tick 0..12; p0 <= q-2; B window b >= q+3.
* Why this works although P_T must act after A on open and before A on close: on close the
  re-extending A (RB landing at q+2) sees T already placed by a P_T pulse at q+1..q+2 (block
  event order), so the same pulse delay serves both edges.

## Wiring (s2): fc2's core, re-timed
* The lever cannot sit at fc2's (-2,4,0): it quasi-powers the top-left puller (-2,2,0) through
  (-2,3,0) (the puller then never retracts). Lever moved to (-2,4,1) (wall, on (-2,4,2));
  observer V (-2,4,0) watches it, O_a (-1,4,0) watches V (p0 at 4), R_q (-1,4,1) delay 3 reads
  the lever (lever tap): Q at 6, so p0 <= q-2 holds.
* B chain as fc2 (O_b, J, R_B d4 into B's quasi cell, O watching R_B into X): B at 14.
* P_T + top/middle right: observer (3,4,1) watching RB's LANDING cell (2,4,1) (one change per
  edge: RB arrives on open, leaves on close, at q both times) -> block (4,4,1) -> dot dust
  (4,4,0) -> its floor (4,3,0) powers P_T and the top-right puller: fires at q+2 = 8.
* Left column: observer o (-3,4,0) watching V, observer (-3,3,0) -> cross dust (-3,2,0) on
  (-3,1,0) (stack feed gadget).
* P_BM: observer (2,-1,0) watching BR + block (2,-2,0) (fc2's).

## 09:36Z s3 = 47 blocks, slime-free (BEST.txt; archive/fresh-d-s3-47b.txt), opens 26 / closes 14
Verified `--extra-offsets 28`. 21 mechanism + 26 control.
* The bottom-right puller was the hard part: a top feed (block above the top puller) fires the top
  puller and the middle one (quasi-connectivity) but never the bottom one, and any source next to
  the middle puller's cell double-fires the middle. No 2-block addition exists (`door grow`, 750k
  candidates in two boxes).
* Fix (4 blocks): the observer's output block (4,4,1) powers a cross dust (4,3,1) below it whose
  floor is a COPPER BULB (4,2,1); the bulb toggles once per lever edge (rising edges only), so an
  observer (4,1,1) watching it fires exactly once per edge, into a block (4,0,1) next to the
  bottom-right puller (fires at 10; the side pullers take any tick).

## 09:40Z s4 = 45 blocks, slime-free (BEST.txt; archive/fresh-d-s4-45b.txt), opens 26 / closes 14
* `door minimize --moves all` dropped the dot dust (4,4,0): the cross dust (4,3,1) under the
  observer's block already points into (4,3,0) (P_T + top-right + middle-right).
* It also dropped the lever's wall block, which leaves a FLOATING lever (not survival-placeable,
  the verifier does not check supports): rejected. Legit equivalent: the lever hangs on V's front
  face (lever[face=wall,facing=south] at (-2,4,1) on the observer V (-2,4,0) that watches it).
* Note for everyone: the minimizer can remove a lever's support; check `face`/`facing` by hand.

## s4 block budget (45 = 21 mechanism + 24 control), all positions absolute
* Mechanism 21: door 9; left pullers (-2,0..2,0) E; right pullers (4,0..2,0) W; P_BM (1,-2,0) up;
  P_T (3,3,0) W; A (1,4,0) extended + RB (1,4,1) + Q (0,4,1) E; B (1,5,0).
* Core 11: lever (-2,4,1) hung on V (-2,4,0) (observer watching it); R_q (-1,4,1) d3 lever tap +
  floor (-1,3,1) -> Q at 6; O_a (-1,4,0) watching V -> X dust (0,4,0) on (0,3,0) (p0 at 4, and A's
  lower position through the floor); O_b (-1,5,0) -> J (-1,6,0) -> R_B (0,6,0) d4 -> B's quasi cell
  (B 14..22); O (0,5,0) watching R_B -> X (A pulses 16, 24).
* Left 4: o (-3,4,0) watching V, observer (-3,3,0), cross dust (-3,2,0) on (-3,1,0) (stack feed).
* Right + P_T 7: W1 (3,4,1) watching RB's landing cell (2,4,1) (fires at q+2 = 8 on BOTH edges:
  RB arrives on open, leaves on close) -> block (4,4,1) -> cross dust (4,3,1) below it -> points into
  (4,3,0): P_T + top-right + middle-right (quasi); the dust's floor is a waxed copper bulb (4,2,1),
  observer (4,1,1) under it -> block (4,0,1) -> bottom-right at 10 (gadget bulb-pulse).
* P_BM 2: observer (2,-1,0) watching BR + block (2,-2,0).
* `vanilla-check` (2 positions x 2 cycles + real time): PASS, identical tick for tick.

## Searches that found nothing on s4
* `door minimize --moves all --plateau` (2.3k removals/compounds, 440k swaps, 35 neutral designs).
* `door regrow --size 2` (7k, states incl. lamp) and `--size 3` (1.9M candidates, 7 min).
* `door grow` 2 blocks for the bottom-right puller (140k + 614k candidates, two boxes).

## Why the slime-free family stays above fc2 (40)
Per function vs fc2: left column 7 (3 pullers + 4 feed) vs 6 (3 slime + P_L + 2 observers); right
column + T aside 11 (3 pullers + P_T + 7 feed) vs 7 (6 slime + P_R, which gets RB's landing for
free and drags T with its carrier); core, A/B/Q/RB, M and door identical. The mechanism is 4 blocks
cheaper (21 vs 25), the feeds cost 9 more. Everything around A is crowded: A's free neighbors are
X (west), RB (+z) and B (up), so T's side piston can only sit east, and its pulse must come from an
observer on RB's landing cell, which can only output east (4,4,1).

## Layouts examined and rejected (paper + quick tests)
* fc2's lever spot (-2,4,0) quasi-powers the top-left puller through (-2,3,0): it extends on the
  pulse and never retracts (no pull). Hence V.
* P_T on +z (1,3,2): RB at (1,4,1) quasi-powers it through (1,4,2). P_T on -z works ideally
  (sf-mech) but no cheap feed reaches it.
* TL-up / TR-up (the 48's "observer on a door cell + block above the pair", 2 blocks per pair):
  the TL puller's cell (0,4,0) is X's, TR's (2,4,0) is next to RB's landing (2,4,1) (level power),
  and with both corners up there is no cell left for P_T.
* A z-side core (lever on O_a at (1,4,-3), X at (1,4,-1), B chain at -z: saves V) makes Q's feed
  cost +2 (the lever is far from Q).
* Hybrid fc2 right structure + slime-free left: 41. fc2's own lever on O_a (no wall block): R_q then
  has nothing to read.
* An observer watching A to fire P_T: the B dance moves A again (extra pulses push T back into
  A's path).
* `door minimize --depth 2 --plateau-pairs` and a neutral random walk (`--walk`) on s4: nothing yet.
  The walk shows X's floor (0,3,0) may be ANY sturdy block (even an observer): A in its lower
  position is quasi-powered through B's head cell (1,4,0), which X points into, and updated by
  X's second-order updates.

## Next ideas
* A cell next to A is the bottleneck (X, RB, B, and P_T's gap all crowd it). A design whose T
  aside piston needs no cell next to (1,3,0)'s east/west (e.g. T aside in z with the shuttle on the
  other z side and X fed from above only) could free (0,4,0) for the TL-up trick (left = 2 blocks
  per pair).
* Copper bulbs as edge filters: a pulse train (or a level with two edges) through a bulb gives one
  observer pulse per rising edge; might replace pulse-stretch repeaters elsewhere.
* Mirrored pump with the shuttle: holding A on close blocks the pump's late pushes (a pump piston
  P under M can double as M's puller after a U lowers it; the close-time push is blocked by A's
  extended head at the top of the column).
