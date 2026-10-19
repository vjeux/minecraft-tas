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

## 4. More screens (16:40Z), all 0
- pockets3 (explore/pockets runs/): m2 and m2w cross-region KEEP k<=3 over all 23 wiring cells (47.1M each);
  m2 bottom-left 5 cells cleared, every filling of up to 5 cells x 47 states incl. pistons (70.8M screened).
- mine: w1b (wool, U at t11 from the sensor-pushed piston head) KEEP k<=3 over 25 cells (93.6M screened).
- m2xl: an E chain with a LAMP instead of the bulb (E2 at t39) fails: the lamp is a conductor and relays the
  plain sensor's strong power into E's QC cell (-2,2), so E stays extended t5..t36.

## 5. Why c077w / c074 do not help in 8x7
- Their S (-1,3) needs the same one-shot 2-gt pulse at an exact tick. The only one-shot (an observer pushed from
  (-2,4) into (-1,4) by a piston at (-3,4)) can only be fed from (-3,3), and any sensor there powers D: strongly
  through a conductor at (-3,2), or by QC (signal into the cell above D) when (-3,2) is air or glass.
- Every other S feed ((-2,3), QC via (-2,4)) slaves E1 to S (E1 after S), ceil's finding.

# Part 2 (16:43Z, coordinator): a SOLID 7x8 from the t7b family (t7b-ob40 in 8 rows), files in t7b/

## 6. Skeleton and requirements
- t7b/ob40.txt = archive/t7b-ob40-40b.txt; t7b/t8a.txt = ob40 minus row -4 (the row -4 chain, B's repeater hold
  (0,-3) on (0,-4), P_M's observer, the head-pop piston (2,-3)) -> x -2..4, y -3..4 = 56. A' pulse 0 stays real
  (mover (4,-2) + moving observer (3,-2) -> (2,-2), fires t9).
- t8a.ideal (P_M, B, gate ideal) works under the tile phase only at g1 29/30, g2 37 (pm 21, b 27, bw 8).
- t8b.ideal (REAL gate observer (0,-2) facing down, the plan toggles its watched cell (0,-3)): the watched cell
  must change at b or b+1 (A' lands at (1,-1) at b+2, its extend runs at b+3: the gate must still be on) and
  EXACTLY at b+bw (B's retraction: A' lands at (1,-2) only QC-powered and nothing updates it later).

## 7. B: the hybrid (east lift pulse + redstone-block hold), t8r.txt / t8r2.ideal
- t8r: sticky X (-2,-3) east, redstone block R (-1,-3): X pushes R into (0,-3) (next to B'), the real gate sees it.
  R alone (two X pulses) fails at every timing: the gate sees R's moving block 2 gt (spit) before R powers B'
  (1-gt spits are not reachable by plan inputs; a 0-tick would align it).
- t8r2 (HYBRID): a 2-gt lift pulse into B' at b (here a plan redstone block behind B'), then R pushed in at
  sr >= b and pulled out at sr2 >= sr+5: B' re-extends under the lifted A' while R sits there, retracts when R
  leaves; the gate sees R arrive (gate pulse 1) and leave (pulse 2, aligned). 3175 of 15675 window combos work
  (b 14..30 with pm 12, sr >= b, sr2 >= sr+5).

## 8. The lift pulse from the right (t8t.txt, ideal-rs --vary over 4 right cells)
- observers (4,-3) facing up (watches the mover (4,-2)), (3,-3) E, (2,-3) E: B' pulses at t11 (A' moving:
  blocked), t15 (the lift), t19 (harmless). With X ideal: WORKS for pm = 12 exactly (BM lands at (1,-1) at t13,
  so P_M's pull needs pm >= 12; the lift at 15 needs pm <= 12), sr 15..18+, sr2 >= sr+5 (156 combos).
- With a B'-watcher gate (observer (0,-3) E on B' -> gate) instead of R: 0 over the same 4 right cells (t8w).

## 9. P_M from the left (t8p.ideal: B ideal, ideal-rs --vary over 5 left cells, 17 states)
- 26 (b 18) / 41 (b 22) of 1.4M work, all with (-2,-1) observer facing up on BL's puller (fires t9, t13), e.g.
  -> (-2,-2) lamp -> (-2,-3) piston east (head into (-1,-3)) -> (-1,-2) observer facing down -> P_M.
  They use (-2,-3) and (-1,-3), the R shuttle's cells: the combination is the open problem.
- P_M (-1,-1) can only be fed from (-1,-2) (an observer facing down watching (-1,-3), or a conductor) or
  (-2,-1) (a conductor there also fires BL's puller (-2,0); an observer facing west there watches outside).
