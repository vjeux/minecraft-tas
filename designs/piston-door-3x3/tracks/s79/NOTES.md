# Track s79: a 7x8 = 56 door from the COMMUNITY 7x9's mechanism (c79)

Box x -2..4, y -2..5 (door at 0..2, 0..2). c79's mechanism fits it (10 pistons + c79's signal piston
ND(3,-2), which is wiring); only its bottom wiring needs row y = -3.

## 1. c79's mechanism + c79's top wiring in 7x8, bottom ideal (02:05Z)
`c78m.txt` + `c78m.ideal`: c79 with row -3 and the bottom wiring removed (R0 (4,0) kept), BR's puller
BRp (2,-2), U (1,-2), K (-1,-1), L0 (-2,0) ideal at c79's own ticks (plan t = piston acts at t+1).
`door ideal-rs` WORKS (4 positions x 2 cycles), open 14 / close 14, frame 12/12.
- Same-tick order matters: at plan 10 K's clear must be applied before U's set (c79: K's retraction is
  queued before U's second extension in tick 11), else U pushes BM + C back up.
- Windows (independent sweeps, 2 gt pulses, others at c79's ticks):
  BRp any (0..30) both edges; L0 any both edges;
  K open 4..8, close 7..9; U1 open 2..5 (blocked by s's head: harmless) or 10..13, close 0..5;
  U2 open >= 10, close 9..11 or >= 14.
- The symmetric train that fits both edges: U1 @3..5, K @7..8, U2 @10..11 (+ BRp, L0 anywhere).

- Joint sweeps (`--sweep` grids, /tmp): CLOSE is flexible: U1 any, K >= U1 + 3, U2 >= K + 1 (gaps);
  OPEN: K >= 2, the effective U pull >= K + 2, and every earlier U pulse must fall in plan 2..5
  (blocked by s's head) or be swallowed by C still moving.
- `c78m-h3.ideal` (s held on open by an ideal block until plan h, M's repeater (2,5) delay 3 = M at
  t14): the first U pulse may then come as late as plan 8 (h = 10). M at t16 (delay 4) breaks.

## 2. Pocket search: the 4-cell bottom-left cannot drive U, K, L0 (02:11Z-)
`door cosearch pockets` (assist c78m.ideal, k = 4 = every cell, fill palette all + sculk +
calibrated + wool + amethyst, 19M fillings, runs/pk1.log): fillings drive only {L0, U2} (best 4
blocks, runs/pk1/pocket0-set*.txt); NO filling drives K or U1, and none drives all of U, K, L0 (the
pocket screen re-runs a feeding filling with those inputs released, so a complete bottom-left would
have been kept whatever its timing). Why, by hand:
- feeders: U only from (0,-2) (its QC cell's neighbours are frame cells; (1,-3) is outside); L0 only
  from (-2,-1) (its QC cell is L1: (-1,1) is ML's park); K from (-2,-1), (-1,-2) or (-1,0) (BL's
  park: nothing can emit into it).
- K and L0 both need 1-2 gt pulses (ideal sweep of their pulse width: only 1..2 work; a window on
  L0 re-pulls BL on close, a window on K needs U2 inside it on close and after it on open).
- a short pulse INTO K: from (-2,-1) only dust on a powered (-2,-2) (then L0 has no feeder) or a
  conductor powered from below (no emitter at (-2,-2) can point up but a wall torch: inverted, and
  L0 shares it); from (-1,-2) only a conductor powered by an observer at (0,-2) facing east (then U
  has no feeder); observers pointing into K would watch (-3,-1) / (-1,-3), outside the box;
  repeaters/comparators have no floor there; a sensor gives a 10/30 gt window.
- and on OPEN nothing the bottom-left can watch moves before K must act (U's first pulse is
  blocked by s's head, U's pull must follow K): the open edge needs a wireless trigger.
So c79's mechanism in its own orientation is not wirable in 7x8 with these parts (in-plane or
sculk). The vertical mirror (corner at the top, floors for dust) keeps the sealed corner.

## Why the bottom is hard in 2 rows (by hand)
- The bottom-left (4 cells (-2,-2) (-2,-1) (-1,-2) (0,-2)) sees only U, K, L0, (0,-1), (-1,0); on
  open nothing it can see moves before K must act (U1 is blocked by s's head, U2 must follow K):
  it needs a trigger from outside on the open edge.
- From the bottom-right, U can be QC-powered only through BR parked at (2,-1) (a conductor fed by an
  emitter at (3,-1)); any emitter into (2,-1) also QC-powers BRp, and U is not a neighbour of (2,-1):
  it only re-checks when BRp's base changes, by which time (2,-1) is BRp's moving head. Dead.

## 3. c78x: BR leaving RIGHT (one up-facing sticky in the bottom row), U FIRST on open (02:40Z-)
`c78v*.txt/.ideal`, `c78w*`, `c78x.txt` + `c78x.ideal`: c79 with BRp removed, R0 (4,0) a pull-type sticky
facing west (BR parks at (3,0)), (2,-1) static; U's east cell (2,-2) is then free, so U can be fed from
the bottom-right and the bottom-left can WATCH U from (0,-2). For the bottom-left to see the open edge,
U must move FIRST on open: U (head into the empty (1,-1)) pulls BM down from (1,0), and the elevator's
s pushes only TM + C down afterwards (e >= u0 + 3), with a hold >= 3 gt (push lands, then s pulls TM back).
- ideal-rs: c78x WORKS (open 23 / close 23) with M at t16 ((3,5) and (2,5) delay 4); windows: U-first plan
  <= 5, s open push plan 8..9 hold 3..4 (only these with U first), s close push 0..10, K open 8..18 /
  close 7..19, U2 open >= 13 / close 16..17 or >= 20, R0 and L0 any. M at t14: s push 6..7 (u0 <= 4).
- The c79 top (s pushed at t2) is the "blocked" mode: U's first pulse is then blocked by s's head and
  invisible, so the bottom-left never learns the open edge (it watches only U).
- Bottom-left watching U (observer (0,-2) east -> conductor (-1,-2) -> K): it fires K twice per U pulse
  (U's extension at X -> K at X+2, U's retraction LANDING at X+4 -> K at X+6). On open the X+6 pulse is
  the real pull (BM is still moving at X+4), but on close the X+2 pulse pushes BM in and the X+6 pulse
  pulls it back unless U2 comes at X+4..5, while on open U2 must follow the X+6 pull: edge-asymmetric.
  `--vary` over the 4 cells (honest blocks incl. bulbs, lamps, droppers, sensors, torches, dust): 0 of
  130k drive K (L0 ideal); pockets' 3-block hits use headless extended pistons (not legal play).
- Top: the s open push at t9..10 with a 3..4 gt hold has no source in the top-right (only (3,3) is free;
  (2,4)/(3,4) carry the repeaters; the natural ticks are 2/4 (lever observers), 8/10 ((3,5)), 16 (M)).
