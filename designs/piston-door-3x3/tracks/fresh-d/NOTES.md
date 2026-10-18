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
