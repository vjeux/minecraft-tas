# Track q78: the first 7x8 = 56 door (ring56's q8seed first, sculk allowed)

Brief (coordinator 09:51Z): seeds in order (1) ring56's q8seed, (2) m79s's s78c, (3) mechspace single-U
7x8 layouts; sculk counts (own category). Box: x -2..4, y -2..5 (door at x 0..2, y 0..2).

## 1. q8 = q8seed cleaned (`q8/base.txt` + `base.ideal`)
t8L mechanism (lever ON at rest on TR's puller east face: TR + MR held), left side wired by cosearch run 6's
B-observer -> repeater d4 -> lamps -> observers parity chain; A (1,4) x3, B (1,5) x2, W (3,3), U (1,-2) and
BR's puller (4,0) ideal. ideal-rs WORKS. Windows (one at a time, others at defaults): A1 0..9, W 9..15,
B (b1, b1+12) or (b1, b1+16) only (the left chain's parity), A2 20..27, A3 34..42, BR free, U >= 36.
* B as ONE hold works too (`bhold.ideal`, `bh3.ideal`) with the left repeater at delay 1 or 2: b, length 7..12;
  and with B = a 10 gt hold at b, A at b-8, b+2, b+12 (= a, a+10, a+20: a calibrated sensor's 3 observer taps)
  and W at b-5..b-3. W as a hold (any length >= 3) fails.
* Why the top-right resists (by hand, then screens): A can only be fed from (2,4) (direct high, QC low) and
  (2,4) can only be a full block (nothing supports a repeater there), so A's 3 taps = an observer (2,4)
  facing east watching (3,4) change 3 times; W sits under (3,4) and is fed only through (3,4) (direct or QC)
  or (4,3) (over TR's extended puller: only full blocks, and a sensor or powered block there re-extends
  TR's puller). Anything that makes (3,4) change also feeds W (a sensor holds it, a lamp's power QC-holds it).
  An observer (2,4) facing west watching A gives W two pulses per A move (A's extension and retraction).
* `q8/u1.txt` + `u1.ideal`: U WIRED (ideal-rs WORKS, A/B/W/BR ideal): plain sensor (4,-2) -> observer (3,-2)
  facing east -> wool (2,-2) (wool conducts) -> U; wool (4,-1) occludes the early events (lever, BR's
  puller), so the sensor hears the late elevator (t37: 3 taps t39/t69/t79 = pull, push, pull on open).
  Found by a 229k-variant `ideal-rs --vary` screen of 6 bottom cells. BR's puller then has only (4,-1)
  (wool) as a feed; an observer there watching the same sensor echoes (its 3rd tap re-arms the sensor).

## 2. q8L (`q8/l.txt` + `l.ideal`): t8 with TL LEFT (puller (-2,2)), C0 (0,3) static
Frees (0,4) so A can be fed from the LEFT while W keeps the top-right. ideal-rs WORKS with A at a, a+10, a+20
(a 0..7), W a+3..a+5, B a 7..10 gt hold at a+7..a+8, TL / BL / BR free, U >= a+20.
* `q8/ll.txt` + `ll.ideal` (q8LL): q8L with the lever ON at rest on the WEST face of TL's puller (-3,2): TL + ML
  held (zero wiring), TR + MR (one input (4,2,1)), BR, BL pull-type. ideal-rs WORKS; TR+MR / BL / BR free,
  U >= a+20. Point: the left (A's feed (0,4)) is next to the lever (early), the top-right (W) far (late).
* A WIRED from the left in q8LL (`q8/lla.txt` plain sensor, `llc.txt` calibrated; ideal-rs WORKS with W, B,
  TR+MR, BL, BR, U ideal): sensor (-1,5) -> observer (-1,4) facing up -> observer (0,4) facing west -> A
  (taps T+4, T+34, T+44 plain; T+4, T+14, T+24 calibrated), WOOL (0,5) occludes A / B / W from the sensor
  so its re-arm hears no echo (the 3rd tap is the re-arm tap). Found by a 3.25M-variant screen.
  Windows then: plain: W 9..16, B1 12..20 (>= W+3), B2 39..42; calibrated: W 9..11 + B (12..14, 23..24),
  or W 9..14 + B (16, 29..30), or B as one hold [21..22, +9..16].
* With a WOOL door (static C0 wool) a top-right sensor (4,5) is shielded from the lever / TL's puller at t0
  and hears A1 instead (t7 -> active t10): W = sensor -> lamp (3,5) -> observer (3,4) up works (W at 12);
  but B's feed (2,5) can only watch the same (3,5) (W and B fire together), B from the left needs (0,5)
  (the shield), and TR+MR's only feed (4,3) needs an odd, echo-free pulse source. Open.

## 3. e78: ring56's jf (p2-sculk-r's mechanism) in 7x8 with a LEVER-HELD TOP ROW (04:08Z)
* `e78/e78m.txt` + `e78m.ideal`: jf (top row UP, ML+BL left, MR+BR right, C down 2 by A' (1,-2) on B' (1,-3), BM
  down then into F2 by K (3,-1), static L2 R2 F0) with the top pullers (0,4) (1,4) (2,4) EXTENDED at rest and held
  by the lever ON at rest on top of (1,4) ((1,4) directly, (0,4)/(2,4) by QC through (0,5)/(2,5)): e88's trick.
  Row 5 (jf/p2's observer chain) is gone, the lever is outside the volume: x -2..4, y -3..4 = 7x8 = 56.
  ideal-rs WORKS for EVERY sides time 0..40 x bottom start 0..30 (jf's asymmetric bottom).
* `e78/e78s.txt` + `e78s.ideal`: top AND sides WIRED (observer (-1,4) facing east on TL's puller -> stone (-2,4) ->
  dust (-2,3) -> stone (-2,2): ML direct, BL by QC; mirror (3,4) -> (4,4) -> dust (4,3) -> (4,2): MR, BR), both
  sides at ~t3; only the bottom ideal: WORKS for every bottom start d 0..30. `e78ws.txt`: the same, wool door.
* Bottom windows (`e78sw.ideal`, d = 8): open A1 0..11, K 11..18 (2..6 gt), B1 16..23, A2 21..27, B2 27..32,
  A3 >= 34; close A1 0..11, K 11..17, B1 17..22, B2 >= 24. The fully symmetric train fails (A3 on close pulls BM
  into F1): A2/A3 need p2's gate (F2 powered only while BM is parked there).
* p2-sculk-r's bottom pasted under it (`e78a.txt`, `e78b.txt` with MR+BR ideal) fails: p2's bottom is tuned to
  BL at t2, its two calibrated sensors at t9 (the lever 9.2/9.4 away) and BR at t18; here the sensors hear the
  top pullers at t6..8. With a WOOL door (`e78w.txt`) the top's t0 events are occluded from the bottom: K's
  sensor fires at t10 = BL+7 as in p2, but B''s sensor (0,-3) hears A' (diagonal neighbour, t7 -> t8) and its
  echo comes at t31 (ideal holds show B' needs [7..18, +10] and a 2nd hold ending at 44..46). Screens of p2's
  bottom variants (wool and stone, 300k+): 0. Running: cosearch on the bottom rows of e78s / e78ws, pockets.
* B' as ONE HOLD (`e78bhs.ideal`, `e78bhk.ideal`, `e78bhs30.ideal`): the bottom becomes ONE train for both edges
  (no gate): A1 at a1, K at k >= a1+3 (2 gt), B' held from EXACTLY k+2 for 7..32 gt (10 and 30 both work),
  A2 (A'-high) after, A3 (A'-low) any late tick on both edges (on close it pushes BM+C+TM home; on close B''s
  hold is blocked by K's moving BM and never retries). Two 2 gt B' pulses instead of the hold fail.
  Every B' feed cell ((0,-3), (2,-3), or a conductor there powered from (0,-2)/(2,-2)) is next to A', so a
  B' sensor hears A1 first (+1 tick); dust there points into A'.
* WOOL door, B' WIRED (`e78wsB.ideal` + `--vary` cells, ideal-rs WORKS with A1/K/A2/A3 ideal): calibrated
  sensors (2,-3) facing west and (0,-3) facing east next to B', wool (-1,-1) (and (3,-2) to free A1's tick:
  A1 4/5/7 or 21..24, K = A1+3..4, A2 23..60, A3 late). Their windows (t6..16 and t8..18, they hear the sides'
  pullers / A1) let B' spit A' up at t16 when the first sensor ends (B1), and their re-arm hears A2 at t31:
  windows t33..43 hold B' (head up) and its t43 retraction lowers A' (B2). A1 + K real: not found yet (the BL
  watcher gives A' two pulses; pockets2's left A' filling works in the wool door only without my sensors).
* pockets2's stone partial with only B' ideal (`e78/kb1.txt` = runs/e78s-kb): B' windows (one at a time): open
  B1 17..21, B2 25, 27..45; close B1 17..21, B2 22/24/26: no common (symmetric) pair, no symmetric hold. But
  with NO B' on close it works with an open-only hold [17..20, 27] (or [26..30, +17]) (`kb1ha.ideal`):
  B' must be edge-selective. In a wool door kb1's chains break (sensors hear other events). Screens of the 6 free
  cells with sensors/wool/observers: 0.
* WOOL e78, steps (all ideal-rs WORKS, files in e78/):
  - `e78wsG.txt` + `e78wsG2.ideal`: + GATE observer (2,-2) facing down watching the (2,-3) sensor: its pulses
    go into F2, where BM is parked only on open: A2 (A'-high, t21) and A3 (A' landing after B2, t43, via QC).
    On close F2 is empty, so the close needs one extra A'-low 2 gt push (BM+C home); a symmetric A'-low pulse
    at almost any tick >= 16 works (on open it is harmless).
  - `e78wsK.txt` + `e78wsK.ideal`: + LEFT chain: calibrated sensor (-1,-3) facing west (hears BL's puller at t3,
    active t6) -> lamp (-1,-2) -> observer (0,-2) facing west into A': lamp on = A1 (t8), lamp off = the late
    push (t22). ONLY K is ideal: K must extend at 8..11 and retract at EXACTLY 12..13.
  - K is blocked: its feeds are (3,-2) (below), (4,-1) (east, under BR's puller: anything that powers it
    strongly also fires BR's puller; only dust (not upward) or a west-pointing emitter, which reads outside the
    box) and its QC cell R0. (3,-2) must stay WOOL (it shields the (2,-3) sensor from BR's/MR's pullers at
    t3; without it B' lifts A' at t7 before A1). pockets2: exhaustive k=4 on (3,-3) (4,-3) (4,-2) (4,-1) +
    3 left cells: nothing. Dust at (4,-1) on an observer (4,-2) facing down is a clean K feed, but nothing
    near (4,-3) changes at t8..9 (the right side is shielded by (3,-2); straight down, BR's puller is heard
    at t6).
