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

## Why the bottom is hard in 2 rows (by hand)
- The bottom-left (4 cells (-2,-2) (-2,-1) (-1,-2) (0,-2)) sees only U, K, L0, (0,-1), (-1,0); on
  open nothing it can see moves before K must act (U1 is blocked by s's head, U2 must follow K):
  it needs a trigger from outside on the open edge.
- From the bottom-right, U can be QC-powered only through BR parked at (2,-1) (a conductor fed by an
  emitter at (3,-1)); any emitter into (2,-1) also QC-powers BRp, and U is not a neighbour of (2,-1):
  it only re-checks when BRp's base changes, by which time (2,-1) is BRp's moving head. Dead.
