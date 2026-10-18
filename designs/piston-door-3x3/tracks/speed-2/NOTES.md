# Track `speed-2`: faster solid doors (open 10, open+close < 18), smaller 11/7

Start: the speed track's o7-119b (open 11 / close 7, solid, slime-free, 119 blocks). Ticks as
`door verify` reports them (k = game ticks after the lever flip).

## Odd ticks: where they come from (26.3 source + sim)
* Every scheduled source is even after the flip: the lever acts between ticks (k0), observers,
  repeaters, torches and comparators add 2, 4, 6, 8.
* Odd ticks come only from piston events: the pistons the lever fires move at k1; a block moved at
  k lands at k+2 BE, and a piston updated by that landing acts at k+3 (next block-event phase).
  An observer watching a cell that a k1 piston changes fires at k3 (odd); a chain observer at k5.
* A sticky retract while the pushed block is still moving (same tick, progress < 0.5, or in the
  scheduled phase) spits it: the block is placed in the block-event phase, so a piston that is
  spit into place can extend in the same phase (onPlace).

## Plan for open 10 (from the speed track's p0.drive, validated with scripted inputs)
Q spits A' into E at k6 (no hold), A' extends at k6 and must retract at k7 (power k5..k7: odd),
Q re-extends k9 and pulls A' at k10 (power k8..k10), F 0-ticks at k10 (R_Z landed by k9 BE, after
R_X). R_X/R_Z timing: P_D and P_V fired 2 ticks earlier (k4..k6) -> R_X lands k6 BE, R_Z k8 BE.

## Log
* `o7s.txt` (120, 11/7) = o7 + `shift.patch`: the P_D/P_V chain observer (2,7,-1) watches a new
  lever-net dust (2,6,-1) directly, so P_D/P_V fire at k4 (R_X lands k6 BE, R_Z k8 BE). Same speed.
* **`a1.txt`: 125 blocks, open 10 / close 7 (open+close 17), solid, slime-free**
  (`door verify --extra-offsets 28`: WORKS at 32 positions). a1 = o7s + `a1.patch`:
  * Q's silent hold removed: O_Q's k4..k6 pulse alone -> Q spits A' into E at k6 (block-event
    phase), A' extends in the same phase.
  * **Odd pulse for A' (k5..k7) without any odd source wiring:** a delay-1 repeater (1,4,-2)
    reads R_F's rest cell A (1,4,-1) (the redstone block S_F shoves at k1) and drives stone
    (1,4,-3); the note block (1,3,-3) under it changes state when the repeater falls at k3;
    observer (1,3,-2) (facing north) watches it and fires k5..k7 into stone X (1,3,-1), next to
    E. A' (powered at k6) extends at k6, X's fall at k7 updates it: it retracts at k7, C into N1
    (lands k9 BE). Trick: **a repeater reading a redstone block that a k1 piston moves falls at
    k1+2d (odd)**; a note block turns that power edge into a state change an observer can see.
  * Q's second pulse k8..k10: delay-1 repeater (-3,4,-2) reads the k2..k4 stone (-2,4,-2)
    (k4..k6), observer (-3,4,-1) (k6..k8) powers note block (-3,4,0), observer (-2,4,0) facing
    west fires k8..k10 into Q's QC cell (-1,4,0) (loud: its fall updates Q). Q's base lands k8
    BE powered -> extends k9 -> retracts k10 pulling A' out of E; F 0-ticks at k10: **open 10**.
* `door route2 --need` (all four nets at once) found 0 routings in 12 min; the hand design did it.
* Gadget `designs/gadgets/odd-pulse` (+ mechanics.md row): the odd-pulse trick on its own.
* **`a3-123b.txt`: 123 blocks, open 10 / close 7.** `door minimize` on a1 removed the old
  hold repeater's floor (-2,3,-1) (124); `door regrow --size 3 --max-open 10 --max-close 7` then
  replaced the re-pulse's repeater + floor + observer by one observer (-2,4,-1) facing east that
  watches O_Q directly (k6..k8) and a smooth stone (-3,4,-1) next to the note block (123).
* **`a4-122b.txt` = BEST: 122 blocks, open 10 / close 7.** regrow again: the k2 relay stone
  (-2,4,-2) is unused now. Then a full `minimize --moves all --plateau` pass (2.4M swaps) and a
  65-min `regrow --size 3` (4.7M candidates) found nothing more at 10/7.
* `o7m-118b.txt` (118, 11/7): `door minimize --moves all --plateau --max-open 11 --max-close 7`
  on o7 (60 min): dust (1,6,-2) + its floor -> one smooth stone (R_s and the lock repeater read the
  block the lever dust points into). The same edit breaks a1 (the shift dust re-shapes (2,6,-2)).
  A 35-min `regrow --size 3 --max-open 11 --max-close 7` on it (3M candidates) found nothing.

## Final report (speed-2)
* Best: **`a4-122b.txt` = BEST.txt, 122 blocks, solid, slime-free, open 10 / close 7** (the first
  solid door under 11 ticks; open+close 17 < 18). Also `o7m-118b.txt` (118, 11/7).
* Mechanism: the speed track's o7 (T up into E at k1, Q shoves [A', T] east at k4, A' pulls C
  into N1, Q pulls A' back and F 0-ticks C up into E) with a tick taken out of the middle: Q's
  hold is gone, so Q's own 2gt pulse makes it spit A' into E at k6 and A' (powered) extends in the
  same block-event phase; A' needs power that ends at k7 (odd), made by a repeater reading the
  cell the k1 piston S_F shoves R_F out of (falls k3) -> note block -> observer -> stone next to E
  (k5..k7); Q gets a second pulse k8..k10 (observer watching O_Q -> stone -> note block ->
  observer into Q's QC cell) to re-extend at k9 and pull A' at k10; R_X/R_Z are fired 2 ticks
  earlier so F's 0-tick is armed by k9 BE.
* Failed: `route2 --need` (0 routings for the 4 nets); open 9 (a9: perfect up to k8, then the
  E-watcher's dust also updates F, which pushes [A', C] down before Q pulls A'); close 6 (needs
  F 0-ticking at close k1 from a removable redstone block and TR pushing T into E in the same
  phase: TR gets no update there).
* Next: open 9 via a k7..k9 source on Q's side that never touches F's neighbors (or arm F by
  spitting R_X into R1 in k9's block events with a repeater-timed sticky P_D); close 6 via a BUD
  piston at (2,4,0) facing east whose head entering (3,4,0) updates TR right after F's spit (order
  problem: F's extension updates it first).

## Open 9: what it needs (worked out, not built)
Q spits A' one tick earlier (k5), so A' pulls C at k6, Q's base lands k7 BE, Q re-extends k8 and
pulls A' at k9, F 0-ticks at k9. R_X (k6 BE) and R_Z (k8 BE) are already early enough.
* Q1: Q must extend at k4 (after T lands at k3 BE) and lose power in the k5 SCHEDULED phase (a
  block-event power loss gives retract type 1: Q would pull A' back). Solution on paper: observer
  (-1,3,-1) facing north watching lever-net dust (-1,3,-2) fires k2..k4 straight into Q, observer
  (0,4,0) facing east watching F fires k3..k5 into Q's QC cell; S_a's fall at k4 is the update that
  finds T landed, S_b's fall at k5 is the spit.
* A1: A' power k4..k6: a delay-1 repeater (2,4,-3) reading lever dust (3,4,-3) into stone
  (1,4,-3) makes the note block change at k2, so observer (1,3,-2) fires k4..k6 into X.
* Q2 is the blocker: Q needs power at k7 BE..k8 and a loud fall at k9 (odd). Every odd source
  (k1 movers S_F/F/R_F/T, k3 BE landings, repeaters reading R_F's cells) is north/center; Q's loud
  inputs ((-1,3,+-1), (-2,3,0), observers into (-1,4,0)) are west, and every 1-observer relay
  between them is geometrically blocked (traced cell by cell). An observer watching E fires
  exactly k3..k5 and k7..k9 (the k5 spit re-triggers it) but sits at (1,3,+-1) with its output
  pointing away from Q; the dust relay over (1,4,2) is lit by R_X's rest block above it.
* Built anyway as `a9.txt` (a9.patch on a1: S_a + E-watcher dust net to Q's QC cell, the dust
  also lit by R_X at R2 so Q is powered from rest (blocked by F's head) until k5; A' power via the
  note-block chain retimed to k4..k6; obsidian (4,3,0) against Q's row push). `door drive` shows it
  runs exactly as planned up to k8: Q extends k4, spits A' at k5, A' extends k5 and pulls C at k6,
  Q's base lands k7 BE, Q re-extends k8, A' lands k8 BE. It breaks at k9: the E-watcher's fall
  reaches F through dust (1,4,2) -> (1,4,1) (dust updates its neighbors' neighbors), F (armed by
  R_X since k6 BE) queues its push of [A', C] before Q's pull is queued (HashSet order), and pushes
  them down. Fix ideas: arm F later (spit R_X into R1 in k9's block events by a sticky P_D whose
  fall is a repeater, HIGH priority, so it beats Q's observer fall), or a k7..k9 source on the
  west side that never touches F's neighbors.

## Round 2 (after the coordinator's go): OPEN 9
* **`n2.txt` = BEST: 113 blocks, solid, slime-free, open 9 / close 7 (open+close 16)**,
  rotation-safe (`door verify --extra-offsets 28`: WORKS at 32 positions x 4 rotations).
  Base: lamp-sweep's a1-107 (my a1 re-synthesized). `n1.patch` + `n2.patch`:
  * Q's first pulse moved a tick earlier and made 1 gt shorter at the end: S_a (-1,3,-1), an
    observer watching lever dust (-1,3,-2), fires k2..k4 straight into Q (its fall at k4 is the
    update that finds T landed); O1 (0,4,0), an observer watching F (F retracts at k1), fires
    k3..k5 into Q's QC cell (odd). Q extends at k4 and its power drops at k5: Q spits A' into E at
    k5, A' extends in the same phase. (W + O_Q removed.)
  * X (A' power) k4..k6: repeater (2,4,-3) (delay 1, floor (2,3,-3)) reads lever dust (3,4,-3)
    into stone (1,4,-3) -> note block -> observer (1,3,-2) -> X; A' retracts at k6, C into N1
    (lands k8 BE). (The repeater reading R_F's rest cell is gone.)
  * Q's second pulse k7..k9, west side only (no dust near F or P_X, the a9 failure): O_S
    (-1,4,-1) watches S_F (extends at k1) -> k3..k5 into stone (-2,4,-1) -> dot dust (-3,4,-1)
    powers its floor (-3,3,-1) -> repeater (-3,3,0) delay 1 turns on at k5 -> observer A
    (-2,3,0) watching that repeater fires k7..k9 into Q's back. Q's base lands k7 BE powered ->
    extends k8; A's fall at k9 -> Q pulls A' out of E -> F 0-ticks C out of N1: **open 9**.
  * O1 fires again k11..k13 after F's 0-tick: obsidian (4,3,0) makes Q's row push fail then.
  * Validated first with a scripted Q pulse (`n1.drive`). Close unchanged (7): every Q pulse on
    the close edge meets a moving block or F's head in E (blocked push).
* **`n3-112b.txt` = BEST: 112 blocks, open 9 / close 7.** `door regrow --size 2` on n2: the
  repeater + floor feeding the note block become one observer (2,4,-3) facing east watching the
  lever dust (3,4,-3) (k2..k4 into stone (1,4,-3); the note block changes at k2, X k4..k6).

## Open 8: VALIDATED with scripted inputs (e1.drive), not wired
`e1.txt` = n3 + `e1.patch`. Idea: Q 0-ticks at k4 instead of spitting at k5, so A' is placed
in E at k4, pulls C at k5 (lands k7 BE), Q's base lands k6 BE, Q re-extends k7 and pulls A' at k8,
F 0-ticks at k8 (R_X lands k4 BE, R_Z k6 BE in the 107 family: already early enough).
* Q's power from rest: redstone block R_Q (-1,5,0) QC-powers Q (its push is blocked by F's head
  until T is in E). Partner P: sticky piston (-1,4,0) facing up (in Q's QC cell, a piston there
  does not block QC; R_Q in front of P does not power P). S_a's fall at k4 updates Q, Q extends;
  P (armed silently by then) is updated, shoves R_Q up, P's setBlock updates Q: Q unpowered,
  retracts in the same phase and finalTicks A' into E (k4). Works (`door drive e1.drive`: N1
  clears at k8).
* Real P arming also works (e2): cross dust (-2,5,-1) on O_S's output stone (k3..k5) + stone
  (-1,5,-1) next to P's quasi cell; no dust within 2 of P, so silent.
* Missing for a real door: (1) A' power ON at k4, OFF at k5 (odd fall): needs an observer pointing
  into X (1,3,-1) that watches a cell changed at k1. A lever-fired piston at (3,3,-1) watched by
  an observer at (2,3,-1) (e2) gives the pulse but disturbs TR (3,3,0): TR, quasi-powered k2..k4,
  gets updated, extends into (2,3,0) and blocks Q's push. A torch at (1,3,-1) (on until 2 gt
  after its attachment rises at k3) would do it if (1,3,-2) could be powered at k3; comparator
  subtraction off R_F (tick-study `comparator-odd`: output back at k3) is the other lead.
  (2) Q's second pulse k6..k8 (even) through Q's back or south side only: every QC route now
  ends in P. (3) Recheck the close with P, R_Q and the new pulses.

## New rule (main f3c5884): the 3x3 columns in front of/behind the door must be empty
* n3 broke it only through the lever-net staircase at z=-4 (6 blocks in x 0..2, y 0..2).
* **`c1.txt` = BEST: 112 blocks, open 9 / close 7, compliant, solid, slime-free,
  rotation-safe** (`door verify --extra-offsets 28` from merged main: WORKS). `c1.patch`: the line
  is re-laid over the top at y=4/5 (S (-1,3,-3) -> (-1,4,-4) -> (0,4,-4) -> (1,5,-4), one step up
  to stay off the repeater-powered stone (1,4,-3), whose back-feed would latch the lever net ->
  (2,4,-4) -> M (3,3,-4)); same dust lengths, same block count.
* Tool: `door route2` now treats the columns at any distance as forbidden (it routed into them
  before and the judge threw every such candidate away). Its rip of the 6 cells still found no
  route (the hand route needs a step up over a powered block).
* **`c2-109b.txt` = BEST: 109 blocks, open 9 / close 7, compliant** (minimize on c1): X's pulse
  (k4..k6) comes from the observer (0,3,-1) watching S_a, so the repeater + stones that fed the
  old note-block chain are dead weight.

## Final report (round 2)
* Best: **`c2-109b.txt` = BEST.txt: 109 blocks, solid, slime-free, rotation-safe, compliant with
  the front/back column rule, open 9 / close 7 (open+close 16)**. Pre-rule: n2 113 / n3 112 (9/7),
  a1..a4 (10/7, 125 -> 122), o7m-118b (11/7).
* Open 9 mechanism: Q gets S_a (k2..k4, its fall is the update after T lands) + an odd k3..k5 pulse
  (observer watching F, which retracts at k1) into its quasi cell: Q extends at k4 and spits A'
  into E at k5; A' (X k4..k6) pulls C at k6; Q's second pulse k7..k9 comes from an observer on S_F
  (k1) -> stone -> dot dust -> floor -> delay-1 repeater (on at k5) -> observer into Q's back, all
  on the west side so no dust update reaches F or P_X; F 0-ticks at k9. Obsidian (4,3,0) blocks
  the late Q push after F's 0-tick re-fires the F observer.
* Did not work / open: open 8 validated with scripted inputs only (e1.drive: Q 0-ticked at k4 by
  partner P shoving its quasi redstone block R_Q); missing a k4-on/k5-off A' source (every observer
  on a k1 change points away from E or disturbs TR) and a k6..k8 Q pulse that avoids P. Close 6
  (F 0-tick at close k1 + TR in the same phase) not attempted beyond analysis.
* Tools: route2 `--need` takes '|' between phases; route2 forbids the door's columns; gadget
  `odd-pulse`.
