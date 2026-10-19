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

## 3. DE and S_R+ wired; PN's b/c window and P3's tick left (pn21.txt + ppn21.ideal: ideal WORKS)

* **S_R+ can be a pull-type pulse piston** (pn13/ppn13: retracted at rest, (4,5) empty, one 2 gt pulse
  per edge pulls K_R up / pushes and spits it back): WORKS; or keep it level-held (below).
* **Every top input as a 2 gt pulse** (pn15/ppn15.ideal WORKS): PN a/b/c, DE e1 (push + spit) and
  e2 (pull), P3, L1/L2, S_R+. Windows (ideal): e1 11..17, e2 21..24 (open), b in [e1-1, e2-5],
  c >= e2 + 2; e1=11, e2=19, b=13, c=21 WORKS on both edges (`--set`), and b = e1, c = e2 fails.
* **DE's two pulses are real**: E (-1,6) delay 4 (W's pulse stretched to t10..18), an observer O_D
  (0,6) facing west WATCHING E fires at t12 and t20 into (1,6) = DE's quasi cell (pn17b/ppn17b).
* **S_R+ level from O_D**: a delay-4 repeater R1 at (1,6) fed by O_D MERGES its two pulses into ONE
  window t20..t30 (a pulse arriving while it is on is swallowed), so the waxed bulb (2,6) after it
  toggles once per edge; comparator (3,6) -> S_R+ (level-held, as in f131F-b2). pn21/ppn21: WORKS
  (redsim 4 x 2) with only PN's window, P3 and L1/L2 ideal; windows: PN window start 10..14 open /
  11..14 close, length 10..24 / 8..15; P3 6..9 / 6..8; L1/L2 any.
* A DE watcher at (0,5) (observer facing east) cannot feed PN when DE is pulsed: each DE pulse
  changes DE's base 3 times, the watcher fires twice per pulse and D1 gets extra shots (pn17c,
  ppn12/16 BROKEN). PN fed at e1 and e2 plus one late shot (>= e2+2) also WORKS (ppn17d), and so
  does a PN WINDOW from about e1 to >= e2+2 (ppn17g/17h).
* P3 + L1/L2 by pocket search (pk17h/pocket0-set0): observers (-2,5), (-2,4) facing up and a target
  (-2,3) (the target powers P3 and L2, L1 by QC) give them at t6: L1/L2 accept it, P3 is 2 gt early
  (D1's shot-a block lands on (1,3) only at t8); P3 needs t8 = three observer hops from W, and the
  third hop cannot face P3 in this corner.
* Still missing: PN's window (or b at e1..e1+2 and c >= e2+2). PN's feeds are (-2,5), (0,5) and its
  quasi cell (-1,6) (via (-2,6) = W and (0,6) = O_D); O_D's pulses + 2 would be exactly b and c, but
  the only cells that can watch O_D are (0,5) (its output then goes into (0,4), D1's trigger) and
  (1,6) (output east).

## 4. PN and DE share E's window through a dust dot: only P3 and L1/L2 left (pn26b.txt, ideal WORKS)

* The annealer (`cosearch search`, seed pn21b, focus x -2..4 y 3..6, assist PN window + P3 + L)
  found it in 5 min: **(0,6) = a redstone dust DOT (power 0 at rest) on (0,5) = glazed terracotta**.
  E (-1,6, delay 4) feeds the dot t10..18; a dot powers the block under it, and that block powers
  BOTH its side neighbours PN (-1,5) and DE (1,5) (pistons take power from a weakly powered block),
  while the dust's updates (two blocks around) wake them: PN and DE get one SHARED window t10..18,
  W's pulse still gives PN its first shot (a) through (-1,6). S_R+: comparator (1,6) reads the dust,
  feeds the bulb (2,6), comparator (3,6) -> S_R+ (level-held). Its bottom changed too (note block
  (-2,-2) + observer (-2,-1) -> L0, comparator (3,4) in subtract mode, dropper (2,5)).
* pn26b (the find without two junk blocks in the left column) + ppn26b.ideal: WORKS in redsim 4 x 2
  with only P3 and L1/L2 ideal; open after 24 ticks, closed after 27.
* P3 windows (ppn26c sweep, ideal ticks): open needs P3 to RETRACT at 8..9 (pull), any start; close
  needs it to extend at 4..9. Windows working on both edges: [4,9), [5,9), [6,8), [7,9), [8,9).
  L1/L2: any tick.
* Not wired yet: P3. From W (t2) the left column gives an observer pulse at t4 (one hop) or t6 (two
  hops, then a conductor at (-2,3)); t8 needs a third hop and the column has no room. Ideas:
  P3 armed through its quasi cell (-1,4) (a conductor at (-2,4) powered by an observer does not
  update P3) and retracted by the update of PN's head entering (-1,4) at t10.

### Why P3 resists (pn26b, ideal + traces)

* P3 must pull the top-middle after it lands on (1,3) (shot a: D1 retracts t6, block lands t8 in the
  block-entity phase) and before DE pushes D1 into (1,3) at t10 (start of the shared window).
  So on OPEN P3 retracts at t9 (from a block-entity-phase update at t8: its own head finishing, the
  block landing next to its head, or L2's block landing on (-1,2)) or at t10 before DE.
  On CLOSE P3 must not touch (1,3) while D1's shot-a head is there (t4..t6) and must stay out until
  >= t10 (else it pulls the block back): extend at t6..t8, retract >= t10.
* From W (t2) the only observer hops in the left column are (-2,5) facing up (t4, output into
  (-2,4)), (-2,4) facing up (t6, output into (-2,3)), (-2,3) facing up (t8, output into L2): the
  t8 hop can only feed L2/L1, P3 gets t4 (quasi cell via a conductor at (-2,4)) or t6 (conductor
  (-2,3)); a t6 pulse ends with an update at t8 before the block lands (pn20/pk17h: BROKEN).
  Watchers of PN fire twice (t4 and t8, and again at t12...), watchers of L2 twice (t_L+2, t_L+6).
* A conductor at (-2,4) arms P3 through (-1,4) WITHOUT updating it (the observer updates only
  (-2,4)'s neighbours), so P3 then moves on the next update it gets: PN's head leaving (-1,4) at t4
  (too early on close: pn28a traces P3 at t4 and back at t7), entering at t10 (after DE's push in
  the same block-event phase), or L2's moves through (-1,2).

## 5. Best partial: pn31.txt + ppn31.ideal, only P3 ideal (redsim WORKS 4 x 2, open 24 / close 27)

* L1/L2 wired (main's `door ideal-rs --vary`, 4116 variants): observers (-2,5) and (-2,4) facing up
  (W -> t4 -> t6) and a redstone lamp (-2,3) (a conductor while powered): L2 direct, L1 through its
  quasi cell, at t6. The lamp also powers P3 at t6..t8; with P3's ideal pulse at ideal 6..7 (open)
  and 4, 6..9 or 18 (close) on top, the door works.
* P3 alone (what is missing): it needs power that lasts until real t9..t10 on open (pull after the
  top-middle lands at t8, before DE's push at t10) and must not reach (1,3) before t5..t6 on close.
  A second P3 pulse later is harmless (ppn32: open 10..17 or 20..23, close any).
* Exhaustive `--vary` screens of the left column (-2,3), (-2,4), (-2,5) (the only free cells next
  to P3's feeds (-2,3) and (-1,4)): with P3 and L1/L2 real, 13,824 variants (24 states incl.
  observers, dust, lamps, bulbs, note blocks, targets, pistons, wool, sculk and calibrated sensors,
  torches) -> 0 work; with L1/L2 ideal at any tick 2..12, 3,604,711 variants (31 states incl.
  repeaters and comparators) -> 0 work; with E's delay varied too, 0. The annealer (two runs, 40
  min) found no P3 drive either.
* Why: from W (t2) the column gives t4 (one hop) and t6 (two hops) into P3; the third hop (t8) can
  only face L2. Any pulse into P3 that ends at t8 retracts it before the block lands; arming P3
  through (-1,4) (silent) needs (-2,4) as a conductor, and then PN's head leaving (-1,4) at t4
  extends P3 too early on close, while PN's head entering at t10 comes after DE's push in the same
  block-event phase.
