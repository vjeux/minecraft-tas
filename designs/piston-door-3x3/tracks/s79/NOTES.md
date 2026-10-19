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
- **1 gt first U pulse fixes it (03:08Z)**: if U's first pulse is 1 gt (extend X, retract X+1: on close
  the push is spit, on open the 1-tick sticky retraction still pulls BM from (1,0)), the U watcher sees
  only U's extension (the retraction and its landing at X+3 fall in its busy window): ONE K pulse at
  X+2, which pulls BM on open (landed at X+3 < K's retraction at X+4) and pushes BM on close. ideal-rs:
  never works with a 2 gt first pulse, works with 1 gt for u0 = 2..6 (obsidian at (3,-1) stops K's later
  pushes on open: BM -> C -> static (2,-1)).
- `c78y.txt` + `c78y.ideal`: the bottom-left REAL: observer (0,-2) east (watches U) -> stone (-1,-2) -> K;
  dropper (-2,-2) (triggered by the powered (-1,-2)) -> observer (-2,-1) down -> L0 (3 pulses per edge:
  odd), obsidian (3,-1). ideal-rs WORKS (open 31 / close 31) with only U (1 gt first pulse + U2),
  R0 and s ideal. s open window: e >= u0 + 2, eh 3.., e + eh <= 12 (u0 = 2: e 4..9).

## 4. c78z: the elevator real too; the bottom-right timing wall (03:20Z)
`c78z.txt` + `c78z.ideal`: c78y with the top re-timed for U-first: a LIT redstone lamp at (4,3) replaces
c79's dropper as R1/R2's conductor (the dust (4,4) on it), observer (3,3) east watches the lamp (fires t6
on open, t2 on close) -> static (2,3) -> s pushes TM + C down at t6 on open (spit; on close s sits at
(1,4), not next to (2,3)); observer (3,4) down watches (3,3); observer (2,4) east watches (3,4) -> (1,4):
s pulls TM back at t10 on open, pushes TM at t6 on close; M at t16 ((2,5) delay 4). ideal-rs WORKS with
only U (1 gt first pulse at plan 0..3 + U2 at >= 20 / 16..17) and R0 (any) ideal.
- The wall: U's first pulse must start by tick 4 (s pushes at t6), but the bottom-right (sealed: it
  touches only U, R0, the static (2,-1) and (3,-1)) can first hear R1/R2's t1 move at tick 3 ((4,-1),
  distance 2), and U would have to be powered and updated in that tick's block-entity phase: impossible
  (U's free neighbour (2,-2) is at distance 3.6 -> tick 4, and the cell above it is the static (2,-1)).
  `--vary` over the 4 bottom-right cells + (3,-1) (pistons, redstone blocks, observers, sensors,
  lamps, droppers, note blocks): 0 of 1.17M; pockets k=4: only R0 and U2 (pockets2 agrees).
- Moving the s push later needs a 3+ gt hold starting at t8..10 and ending by t13 (M at t16 is the
  latest the 2-repeater chain gives): `--vary` over the top-right cells, (3,5)'s delay and sensors /
  observers at (-1,3), (-2,5): 0 of 667k.
- JVM `door ideal c78z.ideal`: WORKS (4 positions x 2 cycles), open 35 / close 35 (redsim agrees).
- Why U-first at tick 5 (plan 4) fails: U's head is still in (1,-1) when (3,3) powers s at t6, so s's
  push is refused (no block event is even queued) and nothing updates s after U's retraction; s then
  pushes only at t10 ((2,4)'s pulse) and spits TM at (1,1).

## 5. Next ideas (not done)
- A top whose s push comes at t8..10 held 3+ gt and released by t13 (M at t16 is the latest a
  2-repeater chain gives): needs a source the c79 top-right lacks (all its ticks are even: lever t0,
  observers +2, lamp off +4, (3,5) at 2d); an open-only calibrated sensor (its back fed with the lever
  level at a strength no door event has, e.g. 13) in the top-left feeding (0,3)/(1,4) could give it.
- Or a U-first source in the bottom-right at tick <= 4 with a 1 gt pulse: only something powered and
  updated in tick 3's block-entity phase next to U would do; none exists in the 5 cells.
- The 2 gt alternative to the 1 gt pulse: U pulses X (U-first/U1), X+3 (harmless on open while BM is
  still landing, U2 on close), Z >= 18 (U2 on open, blocked by s + M's head on close): needs an odd
  offset of 3 between two bottom-right pulses (two sensors whose floor-distances differ by 3, one
  hearing U-first itself) and still s's push >= X+2.

## 6. s79b (2026-10-19 10:36Z-): U real from the bottom-right; the 1 gt need was L0's parity
- `door ideal-rs` with L0 made ideal (`--vary=-2,-1=stone` + an ideal L0 pulse, `s79b/w-l0.ideal`):
  U's first pulse may be 1..4 gt on open and 1..2 gt on close. The "exactly 1 gt" wall came from the
  L0 driver (dropper (-2,-2) -> observer (-2,-1)): it fires L0 once per U-watcher pulse, and a >= 2 gt
  U pulse gives the watcher 2 pulses (extension, landing), so 2 U pulses -> 4 L0 pulses (even). K
  itself works with 2 gt U pulses on both edges.
- **U REAL with 3 blocks** (`s79b/b1.txt` + `s79b/b1.ideal`, c78y top with s ideal, L0 + R0 ideal):
  plain sculk sensor (3,-1) (replaces the obsidian: immovable, still stops K's chain; hears R1's t1
  move at distance 2.24 -> active t3..t33) -> note block (3,-2) (strongly powered from above) ->
  observer (2,-2) east -> U: 2 gt pulses at t5 and t35 on both edges. ideal-rs WORKS (open 37 /
  close 37). Windows with it: s open push plan 7..9 with e + eh <= 12 and a REAL hold >= 3 gt (the
  ideal's between-tick clear makes a 2 gt pulse pull, a real observer pulse spits); s close any of
  0..10, 16..18, 20..30; L0 any single 2 gt pulse; R0 any single 2 gt pulse (3 gt fails).
- With the real dropper L0, U patterns of three >= 2 gt pulses work (`s79b/y3.ideal` sweep, e.g.
  t5..t9, t12..t14, t16..t20) but not two.
- **Three blockers left with U real** (screens, redsim `ideal-rs --vary`):
  1. TOP: U moving at t5 (the earliest a bottom-right sensor allows: R1's t1 move is 2.24 from (3,-1),
     activation t3 BE, U acts t5) means s's open push must come at >= t8 (s's push check at t6/t7
     finds U's head in (1,-1): refused, no event, no retry) with a REAL hold >= 3 gt ending by t13
     (M at t16 is the 2-repeater maximum; s must have landed before M's pull). c78z's top gives t6
     (refused) and t10 (2 gt: spit, no pull-back); the spit + re-extend + pull trick from t8 lands at
     t16 (one tick late). 0 of 577k variants of (3,3), (3,4), (2,4) (observers x4, repeaters E/W
     d1-4, lamps, note blocks, dust, sensors, calibrated sensors), (4,4), (4,3), (-1,3), (-2,5) and
     the repeater delays give it (s79 had 0 of 667k for the 1 gt plan). Odd-start holds need a
     piston-timed source ((3,3) facing down watching R2's head fires t3..t5, its 2-observer chain
     t7: one tick short; a d2 repeater from it gives [t7, t11) with no update at t8).
  2. L0: the U-watcher fires twice per >= 2 gt U pulse (extension; landing 4 gt later), so 2 U
     pulses = 4 watcher pulses at t7, t11, t37, t41 on BOTH edges, and every bottom-left L0 driver
     fed from it (dropper, lamp merge, bulb, reshaper) gives L0 an even count; an extra U move to
     fix the parity is always harmful (pushes C back up / pulls BM down) or blocked (invisible).
     Sensor taps for L0 self-echo (the 3rd tap moves L0 right after the sensor re-arms).
  3. R0: needs exactly one 2 gt pulse per edge (3 gt fails: the close push pulls back). Its only
     feeder (4,-1) sees (4,-2) and the sensor window (two edges -> two pulses); a copper bulb
     flips `powered` on both edges (two observer pulses); sensor-phase taps (3, odd) self-echo, and
     a calibrated sensor at (4,-2) back-filtered by the note block is deaf only until t33, then U2
     re-triggers it. pockets (2 cells, k=3, sculk/calibrated/wool, --gaps): only headless-piston
     fillings (not legal play).
- Wool door blocks pass the judge (`closed_problems` rejects only mechanism blocks), so the door and
  the 3 static frame blocks can be wool and occlude vibrations (q78 uses a wool C0 too).
- **Lever in the bottom-right probes** (`s79b/lb1.txt` + `lb1.ideal`, `lb2.txt` + `lb2.ideal`): the lever
  on the EAST face of (4,-1) (outside the box; volume excludes it) makes R0 level-held (extended at
  closed rest, retracts at t1 on open pulling BR, extends at t1 on close): R0 needs no wiring, and the
  bottom-right hears the lever at t0 (the sensor (3,-1) at distance 2: active t2, U at t4). lb1
  (U real, R1/R2 pull-type with ONE ideal stack input at (4,2,-1) - an input at (4,1,-1) QC-feeds R0 -,
  M ideal through a redstone block at (2,5), s and L0 ideal): ideal-rs WORKS; R1/R2 work at ANY tick,
  M any open tick >= 16, s's open push window widens to plan 6..9.
  lb2 = lb1 with U RESTING EXTENDED (head at (1,-1)) and K + L0 REAL: WORKS. U's open move is then a
  retraction (one watcher pulse) and its close move ends extended (one), so the dropper L0 gets an
  odd count. Windows: open retract at plan 0..5 (later with s later), one 2 gt pulse at 8..50; close
  one 2 gt pulse at 0..23 then extend at 10..17 or 20..50 and stay; s open push from plan 3 with holds
  up to 14 gt when M is late (m 20..30). What is missing: (a) U's power must be FAST-OFF / SLOW-ON
  (off by ~t6 on open, on only after K pushed BM in on close: a lever-level AND its delayed copy;
  a lever on (2,-1)'s front face arms U silently on close - its update hits C - but is fast both
  ways), and (3,-1) must stay immovable for K's chain (a repeater there is destroyed by K's late
  push); (b) the top loses the lever: R1/R2 need a pulse, M a level (copper bulb + comparator, c88)
  and s its pulses, all from sound (wool door blocks can occlude).
- JVM `door ideal` (the real game): b1 (U real by sculk), lb1 and lb2 all WORK (4 positions x 2 cycles),
  same tick counts as redsim.
- Next ideas (not done): (1) lb2's U needs FAST-OFF / SLOW-ON power with an indestructible (3,-1); a long
  open U2 pulse avoids K's late chain push (U's retraction seen while C is still moving), which would let
  a delay repeater sit at (3,-1). (2) The top under a bottom-right lever: R1/R2 from one observer at (4,3)
  facing up (R2 direct, R1 by QC) on an odd-change source, M from a copper bulb + comparator at (2,5),
  s's open hold through (2,3) (reaches s only at (1,3), i.e. open) and its close push through (1,4).
  (3) c78t's right side (TR leaving up, (3,2) static, mechspace) frees (4,2): a lever on (4,2)'s east
  face holds BOTH MR's and BR's pullers, so R0 moves at t1 and the bottom-right hears it at t2.
- `door cosearch search` (annealer, 39M screens in 570 s, `runs/top-anneal.log`) on b2 (U real, L0 + R0
  pinned ideal, s REAL from c78z's top) over all 16 non-piston top cells with the full palette + sculk +
  calibrated + wool and lever moves: stuck at 75/79 (the seed's score): no top wiring re-times s.

## 7. c78l (2026-10-19 12:25Z-): U's level F real; only a long open U pulse missing
- `c78l/lb3.txt` = lb2 without U's ideal input; `c78l/lb3p2.ideal` (top ideal, late: e 10, m 26).
- Bottom-right, all REAL: repeater (3,-1) facing east delay 4 (reads the lever block (4,-1), powers
  (2,-1): U by QC), observer (3,-2) east (the repeater's floor) watching a note block (4,-2) (flips with
  (4,-1) at t0), stone (2,-2) -> U. On open F drops at t8 and updates U through its head (U retracts,
  pulls BM). On close the observer pulse extends U at t2 and spits C (t4), and F's rise at t8 is SILENT
  ((2,-1)'s neighbour (1,-1) holds air, U is not a neighbour of (2,-1)): U stays retracted, armed, until
  K places BM in (1,-1) (that update fires U: the column goes up). ideal-rs: WORKS with ONE ideal input
  left, an open-only U pulse (P2) at plan >= 10..11 lasting >= 4 gt (also works symmetric: swallowed on
  close). Open 33 / close 20.
- P2 must last >= 4 gt: a 2-3 gt U2 lets the watcher see U2's landing, K then pushes BM, C, (2,-1) and
  POPS the repeater at (3,-1) (trace: entity at t27). >= 4 gt: the watcher sees the retraction start
  instead, K's push meets C still moving and is refused.
- `--vary` over (2,-2), (3,-2), (4,-2) x 28 states and (3,-1) x 34 (sensors, bulbs, lamps, pistons,
  torches, diodes): 0 of 746k (no 4-cell bottom-right gives P1 + P2 + F: (2,-2) is U's only free feed and
  carries either a 2 gt observer pulse or a 10/30 gt sensor window, never both).
- **Top wired by the annealer (13:25Z)**: `c78l/t2a.txt` + `t2a.ideal` (cosearch search on lb4, 16 top cells,
  sculk allowed, frame cells locked; runs/c78l-top2): M and both s inputs REAL (calibrated sensors (-2,5),
  (3,5), plain sensor (0,5), waxed bulb (2,5), observers, a wall torch, an iron trapdoor). ideal-rs WORKS at
  32 positions and JVM `door ideal` WORKS (open 43 / close 33), frame 12/12. Left ideal: R1/R2 (one 2 gt
  pulse per edge through (4,2,-1), ANY tick both edges) and P2 (open U pulse >= 4 gt at plan 18..28,
  symmetric OK).
- **R1/R2 real by hand (13:37Z)** `c78l/lb5.txt`: plain sculk sensor (3,5) -> waxed bulb (3,4) (the sensor's
  strong-down output toggles it once per lever edge) -> comparator (4,4) facing west -> observer (4,3) facing
  up -> R2 directly + R1 by QC: one 2 gt pulse per edge. Then the annealer re-wired M + s around it in 26 s:
  **`c78l/p5-0..3.txt` + `p5-N.ideal`: the WHOLE door real except P2** (redsim 32 positions WORKS, JVM
  `door ideal` WORKS, open 45 / close 25, frame 12/12, sculk). P2 window in p5-0: plan 12..35, >= 4 gt.
- P2 is the last input. pockets2's exhaustive k=4 x 61 states --gaps over (2,-2) (3,-2) (4,-2) (3,-1)
  (runs/t2a-bot-k4 on explore/pockets): nothing drives P2, not even alone. U's feeds are (2,-2) and QC via
  (2,-1) (F's repeater); (0,-2) is the U watcher (K, L0); (1,-3) is outside the box. A sensor next to U hears
  U's own t2 close spit and keeps U powered (no spit), so sound cannot sit at (2,-2)/(0,-2).

## 8. c78L: TM leaves up-LEFT, C goes up 2, BM stays down: U needs only the lever level (13:46Z)
- The P2 wall comes from BM leaving sideways (K) and C needing a second U pull. New layout `c78l/c78L0.txt`:
  TM (1,2) -> (1,3) (s pull) -> (0,3) (new sticky TP (-1,3) facing east); C (1,1) -> (1,2) -> (1,3) (M (1,5)
  lowers s to (1,3), s pulls, M raises s, s pulls); BM (1,0) -> (1,-1) (U's retraction) and stays. K removed;
  statics (0,-1) (2,-1) (2,3). Close: s pushes C down 1, TP pushes TM back to (1,3), s pushes TM + C down 1,
  U pushes BM up (M only moves on open). Frame 12/12.
- U = lever level: repeater (3,-1) east d1 (or d2) -> (2,-1) QC, plus the old observer (3,-2) on note block
  (4,-2) -> stone (2,-2) as the update kick on close (F's rise alone is silent). L0 from the U watcher (U moves
  once per edge: odd). R0 lever-held, R1/R2 by the sensor chain. ALL of the bottom and the right side real.
- `c78L1.ideal` (works in both input phases, tile and --between-ticks): ideal s (open pulls a1 0..3, a4 12..16 at
  (1,3), a6 22+; close c1 0..17, c6 21+), M (open only, on 8..11 off 18..22), TP (open 5..8, close 3..23),
  L1/L2 (any). The whole open sequence may shift up to +15 ticks. Open 26 / close 26.
- Annealer on the 11 free top cells: runs/c78l-L1top1 (the first run used c78L0.ideal, which only works in the
  tile phase: the annealer screens in the old phase, seed 40/79).
- c78L2 (`c78l/c78L2.txt`): s at home driven by observer (2,4) east watching the R1/R2 bulb (3,4): pulses
  t8 and t38 on both edges (bulb powered t6..t36 by the sensor (3,5)'s window). With M SYMMETRIC (on 15..25
  both edges) the close needs TP after M raises s (`c78L2t.ideal`, TP close 28); with TP early on close, M's
  push shoves s + TM + C down one (the close is then done) but s's late t38 pulse reach-pulls TM back up.
- **L3a (14:08Z)** (`c78l/L3a.txt` + `L3a.ideal`, annealer runs/c78l-L2ttop1): M real (comparator (0,5)
  subtract reading the calibrated sensor (-2,5) through wool (-1,5)) and s at (1,3) real (observer (0,4) up
  on that comparator -> TM parked at (0,3) -> s); only TP and L1/L2 ideal. redsim 32 positions WORKS in both
  input phases. Its windows allow TP SYMMETRIC (t11..12 both edges) with L1/L2 at t12 (`L3b.ideal`): one
  2 gt pulse into (-2,3) at t11..12 would drive L2 (direct), L1 (QC) and TP (from behind). Annealing that
  (runs/c78l-L3top1).
- How L3a closes (trace): s4 pushes C down at t8, TP pushes TM into (1,3) at t12; M's comparator (0,5) turns on
  at t13 and QC-powers s at home (s sits under M), so s itself extends at t14 shoving TM + C down one (M is
  blocked by the extended s and never moves on close); when the comparator turns off at t23 s retracts and
  pulls TM back up to (1,3); s's late O1 pulse at t38 pushes TM down again. On open M acts first (M's event
  is queued before s's QC event).
- TP / L1+L2 wiring attempts (all redsim): one sensor at (-2,5) + observer (-2,4) facing up + wool (-2,3)
  drives TP, L2 and L1 at once (a calibrated sensor there wakes t9 hearing U/the lever, TP fires t11 = in its
  window) but the observer gives 3 taps (t11, t21, t31): t21 is harmless (on open s is lowered: needs an
  immovable (3,3); on close TP's head meets s's extended head), t31 pushes TM back on open and pulls TM out on
  close. A plain sensor hears R0 at t8 (TP t10: one tick early). A sensor next to (-1,4) QC-powers TP for its
  whole window; a sensor next to s powers s. pockets3: 0 of 119.6M (k=4 over the 7 top-left cells, M + s2 kept).
- **L3d** (`c78l/L3d.txt` + `L3d.ideal`, 32 positions, both phases): the left stack L1/L2 AND TP can be
  LEVEL-held (extended at closed rest) by ONE in-plane block at (-2,3): open OFF at 12..14, close ON at 8..16
  or 23..29, i.e. the lever delayed 12..14 gt. No in-plane level source found that reaches (-2,3): top-left
  comparators can only point west (out of the box) or east (at s / M).
- Sensor-shaped TP/L signals (ideal sweeps on L3e/L3f/L3g, (3,3) obsidian helps):
  * TP with TWO taps (t, t+g) on both edges works for t = 11..12 and g = 4..12 or 26..34 (`L3g.ideal`): a
    sensor waking t9..10 -> note block / iron trapdoor / lamp -> observer -> TP (gap 10 calibrated, 30 plain,
    14/34 with a lamp). The second tap is harmless (open: s is lowered in (1,3) and the push of TM, s, (2,3) is
    stopped by an immovable (3,3); close: TP's head meets s's extended head).
  * TP + L1/L2 with THREE taps (t, t+10, t+20, one calibrated sensor + observer into (-2,3)) works with
    t = tc = 11 or 12 only if M is held ~20 gt on open (on 13..14, off 30..36) so the third tap meets a
    lowered s (`L3f.ideal`, M and s2 ideal).
  * satwire3: L3b with L3a's top kept is UNSAT (core: close TP + close s; every TP source in the free cells
    disturbs s's close power).
- 15:33Z: the first 7x8 exists (e78-56b-40-40b wool / e78st-56b-40-40b stone door, 51/15, sculk, by e78 +
  door fill --keep). New goal from the coordinator: a SOLID 7x8, or a faster/smaller sculk one from c78z.
  L3a (44 blocks with 2 ideal inputs) opens and closes in 40 ticks.
- L5a (`c78l/L5a.txt`): TP + L1/L2 + s2 real from a calibrated sensor (0,5) facing west (back to M, so it
  powers neither M nor s by QC) waking t7 on the lever -> waxed bulb (-1,5) -> comparator (-2,5) facing east ->
  observer (-2,4) up -> wool (-2,3): TP, L2 and L1 fire at t11 exactly; observer (0,4) up on the sensor -> TM at
  (0,3) re-drives s lowered (taps t9/t19/t29). Fails on an ECHO: the sensor re-arms at t27 and hears s's late
  t38 extension (diagonal neighbour, no wool can shield it), toggles the bulb again and L1/L2 push ML/TL back
  in at t43. M must then come from (2,5) (not solved).
- satwire runs on the whole top free (bottom kept): c78F (TM up + left) and c78FR (TM up + right, TP2 (3,3)).
