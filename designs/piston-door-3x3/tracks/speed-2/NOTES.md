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
* **`a1.txt` = BEST: 125 blocks, open 10 / close 7 (open+close 17), solid, slime-free**
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
* `o7m-118b.txt` (118, 11/7): `door minimize --moves all --plateau --max-open 11 --max-close 7`
  on o7 (60 min): dust (1,6,-2) + its floor -> one smooth stone (R_s and the lock repeater read the
  block the lever dust points into). The same edit breaks a1 (the shift dust re-shapes (2,6,-2)).

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
