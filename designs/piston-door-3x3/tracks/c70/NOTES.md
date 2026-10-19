# Track c70: ceil's c070x (8x7 = 56) wired: U + M (2026-10-19)

Coordinator 13:21Z: take ceil's c070x / c077w (explore/ceil a14f5443, tracks/ceil/NOTES.md sections 3-5) and
wire the last both-edge inputs inside the 8x7 box (x -3..4, y -2..4). Took over t78's 12:39Z claim
(t78's results: explore/t78 tracks/t78/c070x/NOTES.md: E1+E2 real with 3 blocks, BL/BM real, M through an
observer (-1,3) facing up into the static (-1,2)).

Files: wip/ (designs), ideal/ (plans). Copies of ceil's wip/c070x.txt + ideal/c070x.ideal, wip/c077w.txt + ideal/c077w.ideal.

## 1. The tile-tick phase race is an artefact of the ideal U input (13:35Z)
- ceil's c070x.ideal fails on main (tile-tick input phase) for every u/m, works with `--between-ticks`.
- With U driven by a REAL observer (test observer X at (-1,-1,1) facing south, the plan toggles its watched
  cell (-1,-1,2) at gu: wip/b1mU.txt, ideal/b1mU.ideal), t78's b1m (E1/E2 real, M observer (-1,3) up) WORKS
  for gu 6..23 with M's watched cell (-1,4) toggled at EXACTLY gm = gu + 3 (18/525). The U observer's
  off-tick is scheduled before the watcher's tick, so U retracts before D's push in the same tick.
- Same with M as an ideal redstone block behind (-1,1) (ideal/b1mU-rb.ideal): M on at gu+4 or gu+5, off at
  EXACTLY gu+7 (U taps at gu+2): a 1-tick window, as ceil said.

## 2. D held 30 gt by a plain sensor (-3,0) under D (wip/b1mU-s30.txt, wip/h1.txt)
- Open works as c77 described: the sensor hears the lever at t6 while E1 keeps E busy (blocked), the watcher's
  first pulse after U's pull makes D push E, D holds E at (-1,1) until t36: M's window is t14..~t32.
- But (a) E2 (the E chain's tap at t35) lands while D still holds E (swallowed); (b) with an observer watching
  the E sensor directly, the 3rd tap (t45) does E2 but both sensors re-arm (t43, t46) and hear E2 / BL / BM:
  the door turns into a sensor-driven oscillator; (c) on close D pushes E alone at E1's end (t9, (-1,1) empty).

## 3. M real: a calibrated sensor (-1,4) facing west (13:46Z)
- M-side --vary on t78's b1m (E1/E2 real: plain sensor (-2,4) -> waxed bulb (-2,3) -> observer (-2,2) up; M observer
  (-1,3) up into the static (-1,2)), U by the test observer: the only M found is a CALIBRATED sculk sensor at (-1,4)
  facing WEST (its back is the TL puller (0,4), so it never powers it): it hears the lever at t2, ACTIVE t2..12,
  COOLDOWN 12..22: the M observer taps at t4, t14, t24. With it (wip/m1U.txt, ideal/m1U.ideal) U must tap at EXACTLY
  t11 (or t21) on open; close U anywhere 8..34; BL/BM wide.

## 4. U real: a sensor-pushed observer (13:54Z) -> wip/m2.txt: U + M + BM real, only BL ideal
- U-side --vary (5 bottom-left cells, M real): plain sculk sensor (-3,-1) over a sticky piston (-3,-2) facing east,
  observer (-2,-2) facing down, calibrated sensor (-2,-1) facing west (back on U). The calibrated sensor hears the
  lever at t6 and QC-powers + updates the sticky piston through (-3,-1) (one tick before the plain sensor would):
  the piston pushes the observer into (-1,-2) at t7, it lands at t9 and fires at t11 into U. The plain sensor keeps
  the piston out until t37; BL's sound at 26..30 re-wakes the calibrated sensor so the piston returns at t42 while
  every sensor is busy (no oscillation).
- BM real: c77's 3 blocks (observer (4,-1) up on BR's puller, sticky (4,-2) W pushes observer (3,-2) E into (2,-2)).
- ideal/m2.ideal: only BL ideal: open bl 24..30, close blc 24/25/26/30. redsim 32 positions x 3 cycles WORKS
  (37/37); real game (`door ideal`, JVM, 4 x 2) WORKS.
- BL is the wall: BL's puller SU(0,-2) can only be fed from (-1,-2) (its QC cell's neighbours are U, BM's park and
  BL), and the moved observer sits in (-1,-2) from t9 to t42. A joint --vary of the 5 bottom-left cells (U + BL, M and
  BM real, 7.7M combos incl. observers/repeaters/pistons at (-1,-2)): 0. BL level-held (puller extended at rest,
  wip/m2L.txt) keeps the same window (26..30): the window comes from the sensor loop, not from the mechanism.
