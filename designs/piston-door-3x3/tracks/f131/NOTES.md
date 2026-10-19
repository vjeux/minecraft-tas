# Track f131: finish wire131's f131F-b2 (7x9 = 63b, mechanism 131) into a working door

Brief (coordinator, 2026-10-19 07:45Z): wire131's f131F-b2 (tracks/wire131 on explore/wire131) is wired
except N's train (the iron trapdoor (-2,4), three changes per edge); small edits are exhausted
(cosearch2: all 47 states of (-2,5), every <= 4-cell change of the 9 cells around it; boxproof):
re-synthesize the whole top-left (x -2..1, y 3..6).

Files: f131F-b2.txt / p131F-b2.ideal (wire131's, copied), pn*.txt / ppn*.ideal (this track).

## 1. N is not needed: a normal piston PN at (-1,5) facing down is D1's trigger (ideal: WORKS)

`(0,4)` (observer facing west) only sees `(-1,4)`. Instead of an observer there watching N, `(-1,4)` is
AIR and PN = a NORMAL piston at (-1,5) facing down puts its head into it (pistons do not emit, so
nothing reaches P3's quasi cell). One 2 gt pulse on PN = ONE D1 shot (the head leaves while (0,4)
still has its tick pending: the retract change is swallowed); a window on PN = TWO shots (extend,
retract). So D1's three shots = one pulse (a) + one window (b, c): an OR of two sources, no XOR.
* PN's pulse comes for free: the lever watcher (-2,6) emits into (-1,6) = PN's quasi cell (t2).
* `ppn3.ideal` (pn3.txt): PN's window, P3's pulse, L1/L2's pulse ideal: WORKS (redsim, 4 positions x 2).
* `ppn4.ideal` (pn4.txt, every top input ideal) windows (`door ideal-rs --ranges`, ideal ticks, an
  ideal event at t acts at real tick t+1): PN pulse 0..3; PN window start 14..15, end 22+ (open) /
  14..17, 20..29 (close); P3 7..12 / 7..11; L1/L2 ANY tick 0..40 on both edges (one pulse is
  enough: they are free); DE window start 10..14 / 11..14, end 21..22 / 21..24, 29+; S_R+ open any,
  close >= 17.
* PN's b shot must come with DE's push or 1 gt after it (`ppn6.ideal`: ob in {oe, oe+1} for an
  8 gt DE window): D1 lowered at (1,3) must finish its b retraction before DE retracts (DE's
  retract event runs in the block-event phase, D1's landing in the block-entity phase).
* `ppn8.ideal`: ONE window shared by PN and DE (e.g. a strongly powered conductor at (0,5)) works:
  start 11..20 (ideal), length 7..10, with P3 6..7 for the earliest starts (ow >= op + 3..4,
  op >= oa + 5).
* `ppn10/11.ideal`: PN fed by an observer (0,5) facing east WATCHING DE (b at DE+2, c at DE's
  retraction + 2) works for ANY DE window of >= 9 gt (9..30) starting 11..30, P3 6..9, S_R+'s close
  switch after DE's release (cs >= 21 for a 12 gt window at 12).

## 2. Why the shared window cannot be wired in the plane (satwire UNSAT, and by hand)

The window must start >= 10 gt after the lever watcher's pulse (t2 -> t12) and reach BOTH PN and
DE (their only common neighbor is (0,5); dust at (0,6) on (0,5) would reach both quasi cells).
With the lever at (-3,6) the watcher's output goes into (-1,6) only, so one cell ((-1,6)) is left
for the delay before (0,6): a delay-4 repeater gives t10, 2 gt short (ow 9 needs P3 at an odd tick,
ow 10 an odd window start). A top lever (y = 7) or a left lever at y 3..5 only moves the problem
(the watcher's output then enters PN, P3 or the lever's own quasi cells). `door satwire ppn9.ideal
--fixed pn8-keep.txt --box=-2,3,0:4,6,0 --full-frame` (full palette, 47 lever candidates): UNSAT in
18 s, core = close PN (-1,5), DE (1,5), P3 (-1,3).
A DE window of >= 9 gt (for the DE watcher variant) is also out of reach of repeaters fed by an
observer pulse (a repeater stretches a 2 gt pulse to at most 8 gt and never lengthens a longer one).
