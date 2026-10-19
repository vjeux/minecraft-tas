# Track m56: the first 7x8 = 56 door (u7-m10 family), 2026-10-19 07:04Z-

Box x -2..4, y -2..5 (door at 0..2, 0..2) unless noted. Start: m10top's m10f (everything real except A's hold
and L0) and m79p (7x9, A's hold real via the TR relay, B pull-type).

## 1. L0 is real if U's FIRST pulse is 1 gt (p3, 07:40Z)
`p3.txt` + `p3b.ideal` (U ideal, A's hold ideal): m10f's bottom-left re-wired in its 4 cells:
observer (0,-2) on U -> NOTE BLOCK (-1,-2) under K (K at U+2) -> note block (-2,-2) -> observer (-2,-1)
FACING DOWN -> L0. One L0 pulse per (0,-2) pulse (the observer's busy window swallows the note block's off
edge). With U = a 1 gt pulse then a 2 gt pulse (both edges) (0,-2) fires 3 times per edge: K pulls BM on
its FIRST pulse (BM pulled at U+1 lands at U+3, K retracts at U+4) and L0 gets an odd count.
`door ideal` WORKS (4 positions x 2, frame 12/12). Windows: open u1 0..6 (width 1), u2 9..24; close cu1
0..7 (width 1), cu2 9..30. A's hold: start u1+2..9, end <= 12.
- With a 2 gt first pulse (cw1=2 / m10f's lamp chain) it fails: K's first pulse comes 1 gt before BM
  lands, its second pulse comes after U2 should have fired, and (0,-2) then fires 4 times (L0 even).
- Pocket search (cosearch pockets, k=4, full fill palette incl. sticky/piston/redstone block) on the 4
  free bottom-right cells (obsidian (3,-1) kept): NO filling gives U a 1 gt + 2 gt train
  (runs/br1/log.txt). Clearing (3,-1) breaks K's late-push stop.
- By hand: every 1 gt source needs a moved redstone block or a moved powered conductor next to U
  (0-tick / spit timing); U's neighbours (0,-2), (2,-2) and its QC cell (1,-1) leave no room for the
  pusher in 7x8.
- Torches cannot be used as 2 gt-pulse inverters here: a torch toggles only if its input is still on
  at its own tick 2 gt later (gadgets/obs-torch.drive: an observer's 2 gt pulse never turns a wall torch
  off), and the bottom rows have no floor for repeaters.

## 2. The u7-m10 mechanism MIRRORED vertically works (mm10, 07:31Z)
`mm10i.txt` + `mm10.ideal`: elevator at the BOTTOM (A' (1,-1) facing up, B' (1,-3)), single U' (1,4)
facing down, K' (-1,3) pulling TM sideways into (0,3), BRp (2,-2) pulling BR down, R0'/R1' (4,2)/(4,1)
pulling TR/MR right, static (0,-1), (3,0), (2,3); box x -2..4, y -3..4 (7x8). door ideal WORKS (open/close
15/15). Why: dust powers DOWN, never up. In the mirror the corner region (top-left, 4 cells) has a floor
row under it: observer (0,4) on U' -> observer (-1,4) -> dust (-2,4) on stone (-2,3): the stone feeds K'
(east), TL's puller L0' (below) and ML's puller L1' (QC) at once (the original's L0 has no such feed).
- `mm10d/mm10h(-i)`: R stack real (lever on the dropper (4,3): R0' direct, R1' QC; lever (5,3) outside
  the box in mm10g/h), top-left real (4 blocks), U' EXTENDED AT REST (level): its first visible change is
  the retraction, so K' (QC-fed by (0,4) through (-1,4)) pulls TM on its first pulse, and (0,4) fires 3
  times per edge (TL/ML odd). obsidian (3,3) stops K''s late push. BRp and B' pull-type in mm10h.
  door ideal WORKS with U' (level drop + one late pulse / early pulse + late level rise), A' hold, BRp, B',
  A''s last pull, BL ideal. With U''s open pulse late (18) A''s hold may start 3..17, length 3..10.
- Pocket search k=3 on mm10h (runs/mh1): bottom-right drives BRp (2 blocks) and B' (3 blocks); bottom-left
  drives BL (2) and A''s last pull (3); top-right (3 cells) drives U''s drop + close pulse (3) but not its
  late open pulse or late close level; nothing drives A''s hold.
- Walls of the mirror (by hand): U' has ONE feeder cell, (2,4) (its QC cell (1,5) is outside the box), and
  needs 4 transitions per cycle with ~12 gt delays (drop, late pulse / early pulse, late level): no room
  in the 4 top-right cells next to the lever and the R-stack dropper. A''s only bottom-right source is
  BR's park (2,-1), which is BRp's QC cell: powering BR re-fires BRp and pushes BR back up.

## 3. The original top is 1 gt short (confirms m10top)
TR relay: TRp drop at t_d (dust: 0, repeater d1: 2), TR lands t_d+2, A fires t_d+3; the relay repeater
(3,3) reads the lever dust (4,3) and turns off at <= 8, so A lands at <= 10; B (fed by (2,5), whose input
(3,5) is also TRp's QC source) moves at <= t_d + 8. A needs a hold >= 3 and B >= A's landing + 1:
t_d + 8 >= 11 and 8 - (t_d + 3) >= 3 cannot both hold. With p3's bottom (A's hold start >= u1+2 >= 5) the
only fit is TRp at t2, A t5..t8, B at t10: one tick early.

## Files
p1/p2 (failed bottom-left variants: bulb, torch follower), p3 (1 gt U works), mm10* (mirror), runs/*.
