# Track speed / wire: real (lever-only) versions of ideal-6

Best: `wire/BEST.txt` = `wire/w5.txt` — `door verify --extra-offsets 28`: WORKS, 140 blocks,
category solid, slime-free, **open 12 / close 11**. Chain of verified steps: w1 (12/19),
w2 (12/14), w4 (12/13), w5 (12/11). Each wN.txt = w(N-1).txt + wN.patch (`door patch`).
Ideal steps: ideal-7..10 (.txt/.sched/.patch), drive probes *.drive.

## Signals in w5 (k = ticks after the lever flip)
* Lever (0,4,-2) on (0,3,-2) at the north side; everything hangs off it.
* F rest power: redstone block R_F at A (1,4,-1). S_F = NORMAL piston (0,4,-1) next to the lever
  pushes R_F to B (2,4,-1) at k=1 (F retracts at k=1, pulls T into E). On close S_F retracts
  without R_F; S_R = normal piston (3,4,-1) pushes R_F back at k=6 (lands k=8 BE). On open S_R's
  push is blocked by S_F's head at A, so its both-edge pulse is harmless.
* Sides + M: torches (-3,1,0) (block above powers the top piston, QC the bottom one), (5,1,0),
  (1,-3,0); their attachments fed by a dust net from the lever floor (one repeater on the east
  branch, delay 1, so the east sides extend at k=4 on close).
* A': observer chain O_L (1,4,-2) k2 -> O_B (1,3,-2) k4 -> O_A (1,3,-1) k6 into E.
* Q: W (-1,4,-2) watches the lever (k2) into G; O_Q (-1,4,-1) watches W -> k4..6 into Q's QC
  cell (noisy: Q extends k4). Silent hold: dust from G -> repeater (-2,5,0) delay 2 (pulse
  stretch) -> X_Q (-1,5,0) powered k6..10, not adjacent to Q. A' landing at k10 updates Q
  through its head -> Q pulls A' at k11. Obsidian (4,3,0) and (-2,3,0) block Q's/TR's push line
  on close (both-edge pulses are then harmless).
* F reach-through: chain (1,4,-3) k4 -> (1,5,-3) k6 -> (1,5,-2) k8 -> (1,5,-1) k10 into F's QC
  cell (1,5,0) (kept AIR so A' at E is not powered). F extends k11 when Q's pull updates it,
  retracts k12 pulling C: OPEN 12.
* F close spit: W pulse dust net extended south to (1,4,2); observer (1,4,1) watches it ->
  k4..6 into F's side. It is scheduled after O_Q, so on open Q's push (k4) runs first and F's
  push fails on the moving A'. On close F pushes C at k4, spits at k6.
* TR: lever-net dust (3,3,-4)/(3,3,-3) -> observer (3,3,-2) k2... OT (3,3,-1) k6 into TR; TR's
  push fails at k6 (F's head in E) but the east top side head landing at (3,2,0) at k6 BE
  re-updates TR -> TR extends k7, spits T into E at k8. F's base lands k8 BE with R_F back ->
  F pushes [T, C] at k9: CLOSE 11.

## What failed / why
* Close spit via O_A powering C at E: a piston ignores power from its front face (F faces E).
* TR on an even tick right after F's spit: the scheduled-phase update sees F's head still in E;
  needed a block-entity-phase update (side head landing) to get k7/k9.
* Router (`door route`) found no path in this crowded build (treats every non-smooth_stone as a
  component); dust nets were routed by hand.
* Close bound with 2gt observer pulses: F push k_a, spit k_a+2, base lands k_a+4, F2 k_a+5:
  close = k_a+7. k_a=2 is harmful on open (F's base lands k3, T's landing updates F before Q).

## BUD close VALIDATED with scripted parts: close 7 (ideal-13.drive)
`ideal-13.txt` = w5 minus F's close observer (1,4,1), OT (3,3,-1) and S_R's last observer.
`door drive wire/ideal-13.drive` (real lever via `use`; `door ideal` cannot drive this design:
its scripted `set` of the lever does not update the lever's floor block, so the dust net never
sees it): RB2 placed at (1,6,0) after open k16 (QC-only for F: silent). Close: k1 S_F's head
leaving A updates F -> F pushes C (BUD works); RB2 cleared before k2; the W-net dust (1,4,2)
updates F at k2 -> spit, C placed at N1 at k2; TR (scripted at k3, spit k4) puts T in E at k4;
R_F back by k4; F's base lands k4 BE -> F pushes [T, C] at k5, placed k7: CLOSE 7.
Still to wire (planned, not built):
* S2 (silent F source, on >= open k15, off before close k2's W tick): repeater R_s into a block
  X at a QC-only spot of F ((1,6,0) or (2,5,0)), input = lever net (falls at k0 -> R_s off at k2
  with VERY_HIGH priority, before W's NORMAL observer tick), held OFF by a lock repeater L_r =
  torch on a lever chain delayed 5 repeater ticks (unlocks at open k14, R_s on k16; relocks at
  close k14).
* TR: extended at rest, powered by a wall torch at (4,3,0) (facing west, attached to (5,3,0) fed
  by a new lever-net branch (4,3,-4),(5,3,-4..-1)); the torch also powers the east top side
  piston, whose k2 extension (queued after F's spit, since the lever's setBlock schedules W
  before updateNeighbours schedules the torches) updates TR after E clears -> TR at k2, T in E
  at k4. On open the torch drops at k2: TR's head leaves (2,3,0) before Q's k4 push.
* S_R at k2: X2 -> stone, dust (3,4,-3) on it, X4 (3,4,-2) watches that dust (fires k2).

## Next idea (superseded by the BUD section above): close ~8
F as a BUD: silent QC power on F only in the settled open state (slow-on), off at k2 on close
(fast-off); S_F's head leaving A at k1 updates F -> push C at k1; the W-net dust (1,4,2) gives
F a pure update at k2 -> spit; TR at k4, S_R at k2, F2 at k6 -> close 8. Missing piece: a
slow-on/fast-off silent source (comparator/lock variants all had a race or a 2gt shift).
