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
