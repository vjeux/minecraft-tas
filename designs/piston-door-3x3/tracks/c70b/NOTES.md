# Track c70b: c070x (8x7 = 56), continuing c70's m2 (2026-10-19)

Base: explore/c70 tracks/c70/wip/m2.txt + ideal/m2.ideal (U + M + BM real, BL ideal; BL window open 24..30,
close 24/25/26/30).

## 1. Constraints read from the m2 timeline (redsim trace)
- M (K (-1,4) calibrated, lever d=2.24) taps 4/14/24 on both edges: lever-locked, K cannot be filtered (its back is the
  TL puller) nor occluded (the lever line runs through (0,5), outside the box). So U = 11 or 21 exactly (M = U+3).
- U (-1,-1) feeds: (-2,-1) or (-1,-2) (QC cell (-1,0)'s neighbours are the watcher (outputs west) and door blocks).
  BL's puller (0,-2): only (-1,-2), and that block must point east without powering U (observer facing west,
  repeater/comparator fed from (-2,-2)). So with BL real, U must come from a static component at (-2,-1).
- Extra U pulses (after the main one) are harmless only when U's push is blocked: open t<=7, 14..19, >=37
  (E or C in (-1,1)); close t<=7, 33..36 (E2's push in progress). Every sensor-driven U source in the
  bottom-left gives 2-3 changes (plain: T, T+30, T+40; calibrated: T, T+10, T+20; a sensor-held piston head:
  T+1, T+31 / T+11), and on close the second one always lands after E2 (U head into an empty (-1,0) -> watcher
  -> D pushes E alone into (-1,1) and spits it). c70's T/V chain is the one clean one-shot, and it needs (-1,-2).

## 2. Wool door blocks (wip/m2w.txt)
- With wool door blocks + frame, the door occludes the lever from the whole bottom-left: K2 and S hear E1 (t5 at
  (-2,1)) at t7 instead, the c70 chain fires U at t12 (fails). Bottom-left activation ticks with wool:
  (-2,-1) 7, (-3,-1) 7, (-2,-2) 8, (-1,-2) 8, (-3,-2) 8.
- wip/w1b.txt: plain sensor (-2,-2) hears E1 at t8 -> normal piston (-3,-2) up -> head into (-3,-1) at t9 ->
  observer (-2,-1) W -> U at t11 (exact); observer (-1,-2) W on the sensor -> BL at t10. The open sequence works up
  to t39, then the sensor's cooldown/inactive taps move BL again and the sensor re-arms on BL's puller (d=2):
  endless loop. (A sticky piston there pulls D's feed stone (-3,0) down on retraction.)

## 3. Exhaustive fills (door fill, redsim judge, any rotation, 1 position screen)
- m2 KEEP mode, k<=3 changed cells over (-3,-2) (-2,-2) (-1,-2) (-3,-1) (-2,-1) (-3,0) (-3,2) (-3,3) (-3,4)
  (3,-1), 42-state palette incl. pistons/sculk/calib/obsidian: 0 of 4.6M screened.
- m2w (wool) k<=5 on the 5 bottom-left cells, same palette: 0 of 147M (39M screened, rest pruned, 138 s).
