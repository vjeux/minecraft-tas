# Track c88: the community 7x9 (c79) mechanism in an 8x8 box (x -3..4, y -2..5)

## 1. Step 1: c79 in 8x8 with an ideal bottom (WORKS)
`s1.txt` + `s1.ideal`: c79's top (lever, level-held MR/TR, elevator A/B, top-left B-watcher dust) and BR's
puller unchanged; row -3 and the bottom-left wiring removed; U (1,-2), K (-1,-1), L0 (-2,0) ideal at c79's
own ticks. One fix: without c79's normal piston (3,-2) nothing UPDATES BR's puller (it is only QC-powered
through (3,-1)), so (3,-1) is now dust (on S (3,-2)): it powers and updates the puller.
- `door ideal-rs`: WORKS; `door ideal`: WORKS 4 positions and 32 positions (`--extra-offsets 28`), open 14 /
  close 14; `door sheet --fill`: frame 12/12.

## 2. Why a top-left-woken bottom cannot be a fixed pulse train (redsim sweeps)
- Windows (s1, others fixed): open K set 2..8, U >= K+2, L0 any; close U1 0..6, K 7..9, U2 9..11 or >= 14
  (U2 >= 14 also works: A's t13 push spits TM into (1,1) and U2 then pushes BM, C, TM up together).
- LATE bottoms work (s2.ideal, s3.ideal: everything after t14-t28) but only ASYMMETRIC by edge: open K then
  U; close U1, K, U2.
- `s1-sym.ideal` (the same U, K, U pulses on both edges), full grid a,b 0..34, c 0..40: every working
  schedule has one U pulse at set tick 2..5, i.e. inside A's hold (A's head blocks U on open). A fixed train
  needs an EARLY bottom-left signal (t3-t6). In 8x8 the bottom-left can only hear: U (fed only from (0,-2)),
  its own pistons, and the top-left via the left column; the top-left only hears B (moves >= t8, after A's
  hold, so the top-left pulse is >= t10). Circular: no symmetric train.
- So the 8x8 needs an EDGE-ASYMMETRIC element at the left: U must get 1 pulse on open (after K) and 2 on
  close (around K). `door satwire` (s2, s3, palette incl. bulb/cmp/note, --keep-wiring): UNSAT in seconds,
  core = close U (1,-2) alone.
- Found asymmetry: an observer at (-1,3) facing down watching TL's park (-1,2) pulses TWICE on open
  (t14, t18: the puller's head enters, then TL lands) and ONCE on close (t14) (probe, redsim). Its output
  goes up into (-1,4) (c79's top-left net), so the top-left net must be re-laid to use it. Same 2-vs-1 for
  every park/floor-cell watcher (the cells where a pulled block arrives), the wrong way round for U.
- Flat circuits cannot lock repeaters (their sides are off the plane).
- Also no symmetric U,K,U train with ANY pulse widths (1..6 each) once every pulse is >= t10
  (`s1-symw.ideal`, 0 of ~2.4M schedules).
- `door cosearch pockets` on s1 + s3.ideal (k=3, the 13-cell left pocket + (-1,3)): L0 (both edges, 2
  blocks), K close (2 blocks) and U open (3 blocks) are drivable; K open and U close are not.
- `door satwire` (whole box re-wired, s3.ideal, bulb/cmp/note in the palette): UNSAT.

## 3. v2: the bottom mirrored to the right (x -2..5): mechanism WORKS, wiring blocked elsewhere
`v2.txt` / `v2r.txt` + `v2.ideal`: BR goes RIGHT into (3,0) (its puller (4,0), extended at rest like
MR/TR's), K' (3,-1) pulls BM into (2,-1), (0,-1) is the third static. Then (2,-2) is free and U could be fed
from the bottom-right, which in c79 hears the lever at t3. Ideal (BR level, U, K', L0): WORKS (redsim, 4 pos),
windows: BR level any 0..20, U1 2..5 (inside A's hold), K' 7..8, U2 10..11 or >= 14, L0 any.
- The bottom-right loses c79's early link: the observer (4,0) on MR's puller becomes BR's puller, and every
  other watcher of the right pullers outputs out of the box; a lever level cannot go down the 1-wide
  column x=5 (dust -> block -> dust carries no power; torches only go up; observers are 2 gt per cell, too
  slow for U1 at t3-6).
- The left then only hears U, whose watcher (0,-2) pulses 3 times on close (t7, t11, t15) and twice on
  open (t13, t17) (probe), but L0 needs exactly one pulse per edge (an extra pull/push undoes BL). A
  merger + copper bulb + bulb watcher does not fit the 5 bottom-left cells (no support row for repeaters).
- `door satwire` on v2 (whole box): UNSAT, core close U + close L0 (L0 at t20: core close L0 alone).

## 4. The lever on the LEFT (s4 -> s19): bottom + left + B real, A and the right side still ideal
Idea: keep c79's block moves but put the lever at (-4,1) on the west face of (-3,1) (outside the box), so
the bottom-left gets the lever's LEVEL directly; the price is that the top and the right only get pulses.
- Left pullers level-held (extended at rest, heads in the left wall): ML (-2,1) by the lever block, L0
  (-2,0) by QC through it, TL (-2,2) by dust (-3,2). Right pullers MR/TR (4,1)/(4,2) and BR's puller
  (2,-2) become PULSED (any time, windows 0..40). B (1,5) pulsed too (retracted at rest): its 2-gt pulse
  lifts A on open and pushes A+TM down (spit) on close. Obsidian (3,-1) blocks K's early close push.
- K real and level-held (s10): dust (-3,0) -> stone (-3,-1) -> repeater (-2,-1) delay 4 -> K.
- U real (s11): dropper (-3,-2) (level) -> observer (-2,-2) -> dropper (-1,-2) -> observer (0,-2) -> U.
  The dropper (-1,-2) is QC-held by K's repeater through K: on open it is still held when the early pulse
  passes (masked, only U2 fires at t10, once K's level drops), on close the hold is gone so U1 fires at t4
  and U2 at t10. That is the asymmetric bottom section 2 asked for, with no top-left signal at all.
- B real (s19.txt / s19.ideal, from pockets' top-left k=5 find on s11): note block (-1,3) over TL's park
  (its instrument flips when TL lands there on open, t3, or leaves on close, t1) -> observers (-1,4),
  (-1,5) facing down -> observer (0,5) facing west -> B (t9 open, t7 close). `door ideal-rs` WORKS 4 and
  32 positions, `door ideal` (game) WORKS 4 positions; windows of what is left: A push ap 0..3 with hold ah
  3..4, A's top pull atp >= 12, right pullers and BR's puller any tick 0..40.
- A's push+hold real instead (s17.txt / s17.ideal, B ideal): observer (-1,3) facing down on TL's park
  (t1 -> pulse t3-5) -> dust (-1,4) (0,4) on the static (0,3); observer (-3,3) on the TL dust -> dropper
  (-3,4) -> observer (-2,4) -> the same dust t4-6: A powered t3-6. WORKS in redsim with B at bb = 8 only
  (t9: the same dust powers A at (1,4) on close, so B must wait for that close push to land at t8) and
  atp = 12 (+ the close hazard emulation, s11h).
- Not both: A needs an odd pulse (t3, from the park observer) plus an even one (t4, from the TL dust
  watcher) in (0,3)/(0,4), B needs an odd chain of 4 stages ending at (0,5) at t9; in the 11 top-left cells
  the two chains need the same cells ((-1,3), (-1,4), (-2,4)) and every rearrangement I tried breaks parity
  (B at t7/t8 is blocked by A's close push, t10 misses A's landing) or QC-powers TL's puller (anything
  powered next to (-2,3), TL's QC cell, re-extends TL on open: s12 was a clock for that reason).
  pockets k=5 (coordinator relay) agrees: A's push/hold is never driven from the top-left.
- The right side: with B PULSED its watcher (2,5) fires twice per edge (t11, t15: B extends, retracts, then
  lands 4 gt later), so the B-watcher net (s15: mirror of c79's top-left net + observer (4,0) on MR's puller
  -> stone (4,-1) -> dropper (4,-2) -> observer (3,-2) -> BR's puller) double-pulses MR/TR (the second pulse
  undoes the pull) and A's top pull (the second pulse is the close hazard after U2 lands). Only bw = 1 (one
  watcher pulse) or a level-held B (s14: windows bb 8..9) gives one pulse per edge; a level at (0,5) needs
  a copper bulb + comparator (the comparator cannot sit on (0,4)'s dust).
- Pockets' per-pocket screens for s11 claimed the right pullers (with sculk sensors) and BR's puller
  (observer (3,-2), sticky piston (4,-2) up, observer (4,-1)) drivable, but the combination verify kept all
  5 inputs assisted, and the BR filling does not drive BR when I add it to s11 (s11 + filling: BR stays).
- satwire on the left-lever designs: UNSAT immediately, core close K (its model does not reproduce K's
  blocked close push under the dropper's QC hold).

## 5. c88b (2026-10-19 09:40Z-): the door works (s26: solid 8x8, 54 blocks)
`s26.txt` (= BEST.txt): `door verify` WORKS, solid, slime-free, flat, rotation-safe, open 34 / close 34,
frame 12/12, no ideal input. Built in three screens on top of s11's real bottom/left (lever (-4,1),
level-held left pullers + K, U's dropper/observer chain):
- Windows first (ideal-rs sweeps): A's push may fire on BOTH edges (coordinator relay), so one symmetric
  chain can drive A; with B LEVEL-held (extended at rest) the B-watcher fires once per edge, which is what
  the right side needs. With pockets2's 2-pulse A (push+spit at t5, re-pull at t11) the B level must drop
  at ideal >= 14 on open; a symmetric T flip-flop fits (rise 14-15 on close works when the top pull
  cannot reach A at (1,3) with an update).
- Top-left (s24, `door ideal-rs --vary` over 10 cells, 15.5M fillings in 8 min, 198 work): observer
  (-1,3)v on TL's park -> lamp (-1,4) (lit t3, dark t9) -> observer (0,4) W into (1,4): A by QC + UPDATE
  (A is a neighbor of (1,4); powering (0,4) itself never updates A: s22 failed that way). B: observer
  (-3,3)v on the lever dust -> stone (-3,4) -> waxed copper bulb (-3,5) (toggles once per edge) ->
  comparator (-2,5) (on stone (-2,4)) -> repeaters (-1,5) d2 (on the lamp), (0,5) d4 (on the observer)
  -> B at t16 on both edges. Game `door ideal` WORKS.
- Right side (s25): B-watcher (2,5) W -> observer (3,5) W (+2 gt) -> stone (4,5) -> dust (4,4) on (4,3):
  TR direct, MR by QC; dust (3,4) on (3,3) -> stone (2,4): A's top pull at (1,4). The extra stage puts
  the pulse after A lands at (1,3) on close, and a powered stone at (2,4) only QC-powers A there without
  updating it (dust (2,4) on (2,3), c79/s15's way, updates A: the close hazard that limited s15).
- BR (s26, --vary over the bottom-right pocket, 2.77M fillings in 43 s, 1 works): observer (4,0) up on
  MR's puller (2 pulses) -> air (4,-1) -> NORMAL piston (4,-2) facing up (QC'd through the air cell)
  -> observer (3,-2) E -> BR's puller: the piston reshapes MR's double pulse into an ODD train (3 pulses
  on open: pull, push, pull), so BR ends right on both edges.
- Dead ends this session: a copper bulb + observer double-fires (the bulb's `powered` flips on both
  input edges, visible to the observer: sculk sensor -> bulb -> observer gave 2 pulses); comparators and
  repeaters cannot stand on row y=-2 (no support) or on dust; the 3-observer B-watcher alternative
  ((2,4) watching (1,4): 3 pulses open / 1 close) breaks at some positions.
- s27 (= BEST.txt, 53 blocks): `door minimize --seamless full+hall` on s26 turned the right side's
  (3,3) stone + dust (3,4) into one observer (3,4) facing east into stone (2,4). Same checks (verify
  32 positions, rotation-safe, FULL + hallway, repeater input 35/35, frame 12/12). `door regrow`
  sizes 2 and 3 find nothing; redsim --vary screens of the top-left (27.9M, air allowed: min 9 + O1),
  the top-right (480k: min 7) and the bottom-left (346k: min 8) find no smaller region.
- 7x8 with this mechanism: the pistons fit x -2..4 (7 wide), but the left puller wall (x=-2, y 0..2)
  splits the left side: TL/ML can only be fed from (-2,3) (TL direct, ML by QC), L0 and the bottom
  (K's DELAYED level, U's masked dropper) only from (-2,-1)/(-1,-2), and one lever (front face or on
  a puller's back outside the box) reaches one half only; the 8x8's column x=-3 is what lets the lever
  block feed both halves (dust (-3,2) up, dust (-3,0) down). K also needs its 8 gt delay exactly
  (repeater delays 1-3 fail in redsim): a repeater into K must sit at (-2,-1) reading (-3,-1). So no
  7x8 for this mechanism; 8x7 is out too (B at y=5, U/BR's puller at y=-2).
