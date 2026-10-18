# Track speed / wire: real (lever-only) versions of ideal-6

Best: `wire/BEST.txt` = `wire/o3.txt` (open 11 / close 7, 135 blocks: o2 minus the 2 dead reach-through observers; see "o2" below). Before: `wire/w6.txt`, 161 blocks,
category solid, slime-free, **open 12 / close 7**. Chain of verified steps: w1 (12/19),
w2 (12/14), w4 (12/13), w5 (12/11), w6 (12/7; w6 = w5 + w6b.patch + w6c.patch + w6d.patch).
Each wN.txt = w(N-1).txt + wN.patch (`door patch`).
Ideal steps: ideal-7..14 (.txt/.sched/.patch), drive probes *.drive.

## w6: the BUD close (close 7)
* F is a BUD: in the open state it is QC-powered SILENTLY by stone X_F (1,6,0) (above F's QC
  cell (1,5,0), not next to F), driven by repeater R_s (1,6,-1) (delay 1) reading lever-net dust
  (1,6,-2). R_s is held OFF by a lock repeater L_r (0,6,-1) (delay 2) fed by a torch
  (-1,6,-1) on block (-1,6,-2) powered by repeater D (0,6,-2) (delay 4) off the same dust: the
  lock drops at open k14, R_s turns on at k16 (after F's base landed at k14 BE, so F does not
  re-extend). Close: the dust falls at k0, R_s turns off at k2 with VERY_HIGH priority (before
  any observer tick of k2); the lock comes back at close k14.
* Close k1: S_F's head leaving A updates F -> F (still silently powered) pushes C into N1.
  k2: W's observer pulse updates the W-net dust (1,4,2), two cells from F -> F (now unpowered)
  retracts with a spit: C placed in N1 at k2, E empty.
* TR (retracted at rest) is silently QC-powered k2..4 by observer OT (3,5,-1) -> stone X_T
  (3,5,0) and k4..6 by OT2 (3,4,2) (watches the W net extended to (3,4,3)) -> stone (3,4,1);
  neither is next to TR, so TR only moves when the east top side piston, now on the direct
  lever net (east repeater removed), extends at close k2: its event is queued after F's spit
  (the lever's setBlock schedules W before updateNeighbours schedules the torches), and its
  head update makes TR push T into E at k2 (lands k4 BE). OT2 keeps TR powered at k4 BE when T
  lands (else TR, updated through its head, would pull T back); TR retracts at k8 against F's
  head (harmless).
* S_R fires at k2 (X4 watches new dust (3,4,-3)): R_F back at A at k4 BE.
* k4 BE: F's base, R_F and T land -> F pushes [T, C] at k5, placed at k7: CLOSE 7.

## Open <= 11: F 0-tick at k11 VALIDATED with scripted power (ideal-14.drive), not wired
`ideal-14.txt` = w6 with O5 and its chain observer (1,5,-2) replaced by stone and a normal
piston P_X (1,4,1) facing up. Scripted: redstone block R_X at (1,5,1) (QC-only for F) placed at
open k8, and a redstone block at (2,5,1) (QC-only for P_X) at k9. At k11 Q's pull updates F ->
F (powered by R_X) extends; F's own setBlock(extended) updates P_X -> P_X pushes R_X up to
(1,6,1) in the SAME block-event phase; P_X's setBlock(extended) updates F, now unpowered -> F
retracts type 1 and pulls C from N1 at k11 (trace: N1 air at k11, i.e. open 11). Close still 7.
Missing to wire it:
* R_X's cycle: after P_X's push R_X sits in P_X's QC neighborhood (P_X facing up always lands
  it there), so P_X never retracts and R_X never returns. Clean geometry: P_X at (1,5,0) (F's
  QC cell; a piston there does not block QC) facing south, R1=(1,5,1), R2=(1,5,2) (outside F's
  and P_X's power sets); X_F powers P_X in the open state and drops it at close k2, and P_X's
  retraction then gives F the k2 spit update (the W dust (1,4,2) can go). But R_X must be
  re-armed R2 -> R1 between open k7 and k10 only: a pusher P_D at (1,5,3) needs a rising-edge-
  only pulse (comparator subtract: back = lever via delay 3, side = lever via delay 4 -> k8..10)
  and the W net must be rerouted off (0,5,3)/(1,4,3), which point into (1,5,3).
* P_X's power must arrive SILENTLY after R_X lands (R_X's arrival updates P_X) and before k11:
  O5's pulse into (1,5,0) is noisy (it would fire P_X at k10 as soon as R_X is placed).

## o2: OPEN 11 (real 0-tick of F at k11) — open 11 / close 7, 137 blocks
`o2.txt` = s4 + o0.patch + o1.patch + o2.patch (`door verify --extra-offsets 28`: WORKS, solid,
slime-free, open 11 / close 7). Validated first with scripted parts (o0.drive, o1.drive).
* P_X: NORMAL piston (1,5,0) facing south, in F's QC cell (a piston there does not block QC: X_F
  (1,6,0) still QC-powers F). O5 and its chain observer (1,5,-2) replaced by stone (they are the
  floors of R_s and of its input dust; O5's output into (1,5,0) would fire P_X early).
* R_X: redstone block resting at R2 (1,5,2) (powers nothing). P_D: normal piston (1,5,3) facing
  north pushes it into R1 (1,5,1) (QC-only spot of F) at open k6, lands k8 BE; P_D's pulse ends k8
  (head gone, R2 free). F's later updates (A' retract k8, A' landing k10 BE before C lands) fail
  to push, so F stays retracted although powered.
* R_Z: redstone block resting at (2,6,1); P_V: sticky piston (0,6,1) facing east, retracted, a
  pulse-toggle: the open k6 pulse extends it into (1,6,1) and retracts it at k8 pulling R_Z into
  (1,6,1) (lands k10 BE): QC power for P_X (neighbor of (1,6,0)), not adjacent to P_X, so silent;
  it must land AFTER R_X (R_X's landing next to P_X would fire it).
* k11: Q pulls A' out of E -> F (powered by R_X) extends; F's setBlock(extended) updates P_X ->
  P_X (powered by R_Z) pushes R_X to R2; P_X's setBlock updates F, now unpowered -> F retracts
  (type 1) in the same block-event phase and pulls C from N1: OPEN 11.
* P_X stays extended (R_Z, then X_F from k16). Close: P_D's k6 push is blocked by P_X's head at
  R1; P_V's k6 pulse pushes R_Z back to (2,6,1) and spits it at k8; R_Z leaving (1,6,1) updates
  P_X's head (at R1, adjacent), which forwards it -> P_X (X_F off since k2) retracts (normal
  piston: R_X stays at R2). F's close is unchanged: close 7.
* The k6 pulse (both edges): O1 (4,5,-2) watches lever-net dust (3,5,-2) (k2) -> O2 (5,5,-2)
  (k4) -> O3 (5,5,-1) (k6) into stone (5,5,0) -> dust (5,6,0),(5,6,1), up to y=7, west along z=3
  to (0,7,2); floor (1,6,3) powers P_D below it, floor (0,6,2) powers P_V.

## Size at equal speed (12/7): s1..s4 (each = previous + sN.patch; s1 from ../w6-134b.txt)
* s1 128: TR's second hold = observer (3,4,0) (in TR's QC cell, facing north) watching S_R,
  which only moves on close (extends k2) -> k4..6 into stone (3,4,1); OT2 + W-net tail gone.
* s2 125: M's torch = wall torch (0,-2,0) beside the M piston, attached to (-1,-2,0), the floor
  of dust (-1,-1,0) (fed via (-2,-1,0)).
* s3 116: F's close-k2 update = dust (3,5,1) (on (3,4,1); lit by X_T k2..4, then by its floor
  k4..6) -> dust (2,4,1) (2 cells from F, floor (2,3,1)); the 12-block W-net tail to (1,4,2) goes.
* s4 109: Q's silent hold = repeater (-2,4,-1) (delay 2) reading G (W's output) into stone
  (-2,4,0) (next to Q's QC cell); the west W-net loop + old repeater go.
Tried/rejected on paper: sticky S_F (its retraction does not neighbor-update A, so F gets no k1
update); torch attachments or floors next to a side piston (dust power on the block powers the
piston); dust next to a torch-powered stone (latches the lever net); east trunk at y=3 (floors
in the walkway).

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
