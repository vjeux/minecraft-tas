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
