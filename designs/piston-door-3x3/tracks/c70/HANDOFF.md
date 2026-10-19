# HANDOFF: track c70 (ceil's c070x, 8x7 = 56), stopped 14:58Z (coordinator: slot reassigned)

## State
- BEST PARTIAL: `wip/m2.txt` + `ideal/m2.ideal`: everything real except BL (the (0,-2) puller).
  redsim 32 positions x 3 cycles WORKS (open 37 / close 37), real game `door ideal` 4 x 2 WORKS
  (bl 24..30 on open, blc 24/25/26/30 on close).
  - top: lever-held ceiling (ceil); right: TR watcher (3,3) W -> repeater (4,3) -> observer (4,2) up (ceil).
  - E1/E2: t78's plain sensor (-2,4) -> waxed bulb (-2,3) -> observer (-2,2) up (taps t5, t35).
  - M: observer (-1,3) up into the static (-1,2), watching a CALIBRATED sensor (-1,4) facing west (lever t2; taps t4/t14/t24).
  - U: plain sensor (-3,-1) over sticky piston (-3,-2) facing east, observer (-2,-2) facing down, calibrated sensor
    (-2,-1) facing west: pushed observer lands in (-1,-2) at t9, fires U at t11 (= M's t14 tap - 3).
  - BM: c77's chain (observer (4,-1) up, sticky (4,-2) W, observer (3,-2) E pushed into (2,-2)).
- NOTES.md sections 1-4 explain each step (tile-phase race = artefact of the ideal U input; D hold; M; U; BL wall).

## Why it is not a door yet (the BL wall)
- BL's puller SU(0,-2) can only be fed from (-1,-2) (its QC cell (0,-1) touches only U, BM's park, BL) and the U
  observer occupies (-1,-2) from t9 to t42. BL's window (24..30) is set by the sensor loop: BL's sound re-wakes the
  calibrated (-2,-1) so the U piston returns while every sensor is busy (else the sensors oscillate).
- Exhaustive: `door fill` on the 5 bottom-left cells (U + BL together, M + BM real, 42-state palette): 0 of 147M.
  k<=6 including (-3,0) (39 states, 4.1G fillings): stopped at item 1.50M / 2.69M, 0 passes (log not kept).
- With M = the calibrated (-1,4) (taps 4/14/24) U must fire at EXACTLY t11 or t21; every bottom-left chain that leaves
  (-1,-2) free gives U at t9/t10/t19/t20/... (sensors hear the lever at 6..8 + observer 2).
- D hold (plain sensor (-3,0) under D, wip/h1.txt): open works (M window t14..t32) but M's later taps re-fire the held
  E (C spit to (1,1)), E2 lands while D holds, sensors re-arm and oscillate; on close D pushes E alone at E1's end.

## Next steps (in order)
1. Find a U chain whose tap is t11/t21 with (-1,-2) left to a static BL emitter (observer W watching (-2,-2)), or an M
   source tapping at U+3 for U = t9/t10 (e.g. the (-1,4) cell fed through a repeater from a (-3,4) sensor needs the
   E chain moved; see NOTES). A chunked `ideal-rs --vary` of the bottom-left with BL real and M as the (-1,4) glass
   toggle (ideal/b1m-bm-u.ideal, sweep gm) tells which U timings are reachable WITH BL; keep each run < ~10M combos
   (a 142M-combo --vary got the node OOM-killed).
2. BL level-held (puller extended at rest, wip/m2L.txt works with an ideal level) via comparator <- bulb: blocked by
   the comparator's 0 output at placement (c77's pitfall) unless a repeater fits.
3. c077w (BL goes left, no bottom BL puller): with real observers it needs the M gate's watched cell (-1,-2) to change
   EXACTLY 3 ticks after S's observer's (wip/w77r.txt + ideal/w77r.ideal: 14 combos, gs 8..20); its top-left still
   has ceil's E1-before-S order problem.

## Running processes
None (fill stopped, no background jobs).
