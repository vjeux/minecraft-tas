# Track wire64: a 1-wide (flat), slime-free, FULL seamless + hallway door in an 8x8 box (64b)

Brief (coordinator, 2026-10-18 22:41Z): wire agent families' 8x8-102 (v1 catalog id, commit 6b2d389)
`SE(-3,1) SE(-2,1) SE(-2,0) SD+(-1,3) SD(0,4) SU(1,-2) SD(1,4) SU(2,-2) SD(2,4) SW(4,1)`, box x=-3..4,
y=-2..5 (door at 0..2): the center exits LEFT through the row-1 elevator E3 (-3,1) + L1 (-2,1), ML is
lifted aside by PD (-1,3) (extended at rest, released on open), TL/T/TR up, M/BR down, MR right.
Record to beat: flat-seam e89b, 72b (8x9x1). Box64's notes (tracks/box64/NOTES.md sections 1-5) are
the starting point: its sym-train plan breaks because the lifted-L1 driver (observer (-2,2) facing
west, the only cell that reaches L1 at (-1,1) on both edges) watches E3's driver and so fires twice.

## 1. The E3 window (ideal inputs, ideal/w102w.ideal): WORKS at 32 positions x 2 cycles

Same 13-event train on both edges, but E3 gets ONE LONG pulse [s3, s5) instead of two 2 gt pulses:
it pushes L1 up at s3, stays extended (head at (-2,1)), and its sticky retract at s5 pulls L1 back.
The lifted L1 then needs exactly one pulse per edge, and a driver that watches E3's input sees ONE
rising change per edge. Inputs: redstone blocks behind the plane (z=1), L1-at-rest and PD through
their quasi cells (a direct input to L1 quasi-powers L0 below it) with a glass poke for the update.
- open = close = s1 L1, PD level at lv, E3 window [s3, s5), lifted L1 at s4, s6 L1, 7 toggles at tg
  (close: s6 is a no-op, the R1 toggle repairs ML/C/MR that the lifted L1 shoved east).
- windows (others at s1=2, lv=8, s3=12, s4=14, s5=20, s6=24, tg=28): lv 7..12; s3 9..12; s4 = s3+2 or
  s3+3 (L1 lands at s3+2 and fires on landing if powered); s5 >= s4+5 (L1's retract must be over),
  s5 19..22; s6 = s5+2..s5+8; all 7 toggles together tg >= 24 (R1 after the close's shove; the
  others are free).
- `door ideal --extra-offsets 28`: WORKS, open 31 / close 31.

Planned left core for the window (to prove as a gadget): dust at (-3,3) standing on a waxed copper bulb
at (-3,2) above E3: the dust carries E3's window; the bulb (a conductor) powers E3 for the whole window
and toggles ONCE per window (rising edge only), so the observer at (-2,2) facing west watching the bulb
fires once per edge, 2 gt after s3 = s4. The bulb is lit when open, unlit at rest (one toggle per edge).

## 2. The real left core (ideal/w102-left3.ideal + w102-left2.txt): WORKS at 32 positions x 2 cycles

Real blocks for the whole left core, ideal inputs only for its three trigger cells:
- note block N at (-3,2) above E3, cross dust at (-3,3) standing on N: the dust carries E3's window
  [s3, s5); N (a conductor) powers E3 for the whole window and changes its `powered` state at s3 and s5;
- observer W at (-2,2) facing west watches N: it fires at s3+2 (the lifted L1, landed at s3+2, extends
  at s3+3: s4) and again at s5+2, when E3 has already pulled L1 back (pull started at s5): no race;
- the second W pulse even does s6 on OPEN: L1 lands at rest at s5+2 while W is on, ML at (-1,2) is
  strongly powered by W, so L1 is quasi-powered through (-2,2) as it lands and fires; on close the PD
  head sits at (-1,2) (not a conductor) and nothing happens (s6 is a no-op there anyway).
  So L1 at rest needs ONE pulse per edge (s1), not two.
- emulation detail: an ideal block placed BETWEEN ticks makes W fire one tick too early relative to E3
  (E3 acts in the next tick's block events); real wiring switches the dust in the scheduled phase,
  so the window is fed through a delay-1 repeater behind the plane (w102-left2.txt, z=1) in the test.
- windows (w102-left3, others at s1=2, lv=8, s3=12, s5=20, tg=28): s1 0..3, lv 7..10, s3 10..13,
  s5 >= 19 (up to 30+ tested), all 7 toggles together: tg 14..15 or >= 19.

## 3. Why 8x8-102 stays blocked: the two bottom regions (concrete)

The plane splits into three wiring regions: TOP (20 cells: E3 via N, L1 via (-2,2)/(-2,3), PD, D0-D2,
R1 via (4,2)), BOTTOM-LEFT K0 = (-3,0) (-3,-1) (-2,-1) (-1,-1) (-3..0,-2) + ring (0,-1) (feeds L0 and,
through (0,-2) or the ring block (0,-1), UM), BOTTOM-RIGHT K2 = (3,-1) (4,-1) (3,-2) (4,-2) (4,0) +
ring (3,0) (feeds UB). L0, UM and UB are pull-type pistons: each needs exactly ONE pulse of <= 2 gt per
edge (a longer window pushes the block back on close and the sticky retract pulls it out again:
wire56's memory-window gadget), or three.
- The lever touches one box cell, and the TOP must have it: PD needs a lever-derived LEVEL that moves
  before s3 (no other region can reach PD's cells, and no link reaches the top before s3).
- K0's only link to the top is E3 (watched from (-3,0)). With the window E3 changes twice per edge
  (extend at s3, retract at s5; the landing is swallowed): two pulses 7+ gt apart. Not mergeable (the
  merger takes <= 4 gt), and the two-flip-flop converter (bulb, comparator, bulb, comparator, observer)
  does not fit: comparators need floors, so only row -1 can hold them ((-3,-1) (-2,-1) (-1,-1)), and
  the second comparator would have to read a bulb at (-1,-1) horizontally, whose other side is the
  ring cell (0,-1) (plain blocks only). With E3 pulsed twice instead (s5 = s3+6) the watcher could fire
  three times, but then W's second pulse races E3's pull (box64's blocker) because W watches E3's driver.
- L0 cannot be fed from the top: its quasi cell (-2,1) only borders (-2,2) (= W, which must face west)
  and swept cells. UM's free neighbors are K0 only ((1,-1)'s neighbors are swept or ring).
- K2's only link is R1 (watched from (4,0)): two pulses 4 gt apart per edge (extend, then the landing
  of the retract). The merger needs a delay-2 repeater fed sideways, but the watcher's output points
  down into (4,-1) and the only repeater cell (3,-1) outputs into (2,-1), UB's quasi cell: UB would get
  the merged 4 gt window, which fails on close. Two raw pulses 4 gt apart re-fire UB as it lands.
- Moving the lever next to K0 or K2 leaves the top without PD's early level.
Variants checked in the catalog: BR right (SW(4,0)) removes K2 but not K0; BL down (8x7-080..083,
SU(0,-2) instead of SE(-2,0)) frees (-2,0) and lets K0 reach L1 at rest and, through the unused ring
cell (-1,0), the lifted L1, but U0 (BL's puller) and UM still need single pulses from regions that
only see E3/L1 (even or many-change links) or R0 (two pulses 4 gt apart).

## 4. 8x8-118 (v2 catalog) and the bottom-elevator members 122-125/128

Both families keep a level-held lift piston (E3+ at (-3,1), extended at rest) behind the center's
puller L1, which rests IN THE RING at (-1,1) and fires twice on open (at (-1,1): C to (0,1); pulled back
to (-2,1) by E3: C to (-1,1)), plus a second elevator: 118 lifts ML up (D at (0,3) in the ring, lift
E5+ at (0,5)); 122-125/128 lift it down (U at (0,-1), lift at (0,-3)).
- Families' own ideal schedules fail in the simulator: their input for L1 at (-2,1) is a block behind it
  (z=1), which also quasi-powers BL's puller L0 (-2,0) below it, so L0 pushes BL back. In the plane L1 at
  (-2,1) has only pistons around it; it fires through its quasi cell (-2,2) (= L2): TL, sitting at (-1,2)
  when open, strongly powered by an observer at (-1,3) facing up, quasi-powers it without touching L2
  (that is L2's front) or L0. The same observer quasi-powers L1 at (-1,1) through (-1,2).
  f118/f118a.ideal and f125/f125a.ideal (that observer real, poked at (-1,4)): WORK at 32 positions x
  2 cycles. The second L1 shot is a landing fire: the observer must be on when E3's pull lands L1 at
  (-2,1) (1-tick window, p6 = 19 with E3 released at 18); the first shot has a 6-tick window.
- Odd links: a LEVEL piston moves once per edge, so an observer watching it fires exactly once (the
  landing is swallowed). In 118 the bottom-left (L0, UM: free toggles there) can watch E3 from (-3,0)
  and the top-right (D1, D2, R1 + R0 by quasi + BUD from one observer at (4,2)) can watch E5 from (1,5).
- 118 (also 119, 126, 127, 129: same top-left) is blocked in the TOP-LEFT: D at (0,3) can only be armed
  through its quasi cell (0,4) from (-1,4), and D at (0,4) only directly from (-1,4); (-1,4) is exactly
  the cell the L1 observer (-1,3) watches. Anything at (-1,4) that changes state when it powers D (note
  block, observer, torch) fires L1 2 gt later, while ML is still in L1's way at (0,1) (D's pull of ML is
  what has to come first): f118b.txt/.ideal (note block N4 at (-1,4) + observer O4 (-2,4)) shows it,
  L1 shoves ML, C and MR east at t4 for every timing. A plain conductor there (no state change) arms D
  silently but leaves the observer with nothing to watch, and L1 at (-2,1) has no other driver
  ((-3,2)/(-2,3) power L2 directly, whose push would shove TL and ML across the top row).
- 122-125/128 (bottom elevator, no such coupling: f125a works) are blocked by TIMING: on open the
  bottom-left must act first (L0, then U pulls ML down) and the top-left next (L1's first shot), then
  E3, then the bottom-left again (lift, U's second shot). Both regions need lever-early signals, but
  their only link (E3, a level piston) changes in the middle of the sequence on open (at t0 on close).

## 5. Minimal change that unblocks 118's top-left: a STOPPER (MR's puller held extended, SW+(4,1))

- Change: MR's puller R1 (4,1) is extended at rest (head at (3,1), a ring cell at rest only: closed
  checks skip the door plane's ring, and when open R1 is retracted with MR in (3,1), so FULL still
  holds; 118 already rests L1 and D in the ring). Open: R1 released at >= t35 (pulls MR), close: any time.
  f118c.ideal (118 + stopper, ideal inputs) WORKS.
- Why it helps: the L1 observer u (-1,3) watches N4 (-1,4), the only cell that can arm D. Every N4
  pulse fires L1 2 gt later; at D's first shot ML still blocks L1's head, and L1's push line
  ML, C, MR now ends on R1's head: a blocked push, nothing moves. When D pulls ML out, ML's departure
  updates L1 while u is still on: L1 fires right then and pulls C. The coupling now produces exactly
  the needed order.
- Real top-left core (f118d.txt): repeater r (-2,4) facing west (input (-3,4)) -> note block N4
  (-1,4) [instrument=basedrum: the observer under it]; observer u (-1,3) facing up; stone J (-2,3)
  (r's floor); observer o (-3,2) facing west = L2's driver (it can watch a lever at (-4,2) outside the
  box). D at (0,3) is quasi-armed by N4 silently (it only fires when TL's moves update it: N4 must be
  on exactly when L2's retract makes TL leave (0,2), and off when TL lands), D at (0,4) fires on N4
  directly; L1 at (-2,1) fires on landing (u on as E3's pull lands it).
- f118f.ideal: WORKS at 32 positions x 2 cycles (open 39 / close 27). Windows (others fixed):
  N4 window start = L2 poke + 2 exactly, end 0..2 later; L1's second pulse = E3 release - 1 exactly;
  D's second pulse >= E5 release + 1 (31..44 with E5 at 30); E5 release 26..33; R1 release >= 35.
- f118g/f118h: L2's pulse and N4's three pulses can be IDENTICAL on both edges (still WORKS); only
  levels and toggles differ: E3+ must re-extend at t0..1 on close but release at t18 on open (an
  edge-asymmetric level: E3 = NOT lever OR NOT lever_d16, two torches), E5+ re-extension on close has
  a window that includes its open release time (8..15, 17..18, 23..32, 34..35), toggles: L0 and UM
  any time on close, D1/D2/R0 most times, R1 0..37.
- satwire: first run UNSAT with core "L2 (-2,2)": its recorder counted the signal into L2's cell from
  its FRONT (TL strongly powered by u), which a piston ignores. Fixed in satwire.rs (record_ideal: a
  piston's direct power is dropped when only its front neighbor emits into it); re-running.
- satwire rerun (front fix): UNSAT, core E3 (-3,1) + L0 (-2,0) on open. Concretely: E3+ needs a lever
  LEVEL (on at rest, off at t18 on open, back on at t0..1 on close), and every cell that can power it
  is taken or couples: (-3,0) also powers L0 (east of it); (-3,2) is L2's driver o (and a conductor
  there would power L2); its quasi cell (-3,2) can be fed from (-3,3) only by dust (on o) or a wall
  torch, and their supports/inputs are (-2,3) = J (r's floor, directly above L2: powering it powers L2)
  or (-2,4) = r; (-4,1)/(-4,2) are outside the box (only the lever may sit there, and a lever there
  powers E3 while ON, the inverse of what E3+ needs). The top-left has 5 free cells left after the
  coupled core ((-3,3), (-3,4), (-3,5), (-2,5), (-1,5)) for E3's asymmetric level, E5's level and N4's
  three-pulse train: not enough, and the two that matter for E3 cannot reach it without touching L2.
- Next for whoever continues: move E3's control to the bottom-left by changing BL's exit to DOWN
  (SU(0,-2) instead of SE(-2,0): frees (-2,0), so a conductor at (-3,0) can hold E3 without touching a
  piston), with the lever near the bottom-left; then the top-left needs an early link for L2 and N4
  (none exists in 118's geometry: E3 changes mid-sequence), or L2/N4 pulses derived from a bottom-left
  trigger through E3... The stopper trick itself is general: a pull-type puller held extended at rest
  turns a "fires too early" coupling into "fires the moment its path clears".

## 6. v3 8x8-131 and 8x8-135 (explore/families 9159d57; memory-wirable, oc + sym plans)

- 131 (box x=-2..5, y=-2..5): `SE(-2,0) SE(-2,1) SE(-2,2) SE+(-1,3) SU(1,-2) SD(1,4) SD(1,5) SD(2,4)
  SW(4,0) SW(4,1)`: 102's elevator turned upright at the top center: DE (1,5) lifts D1 (1,4) down into
  (1,3), P3+ (-1,3) (level, head at (0,3) at rest) pulls T aside into (0,3), C goes up column 1;
  BL/ML/TL leave left (stack L0/L1/L2 at x=-2), M down, MR/BR right, TR up. 135 is its mirror
  (center down, P3+ at (-1,-1)).
- Families' four ideal plans (8x8-131/135 -oc/-sym) WORK in `door ideal` at 32 positions x 2 cycles.
- satwire (time warp, front fix): 131-sym UNSAT, core P3 (-1,3) on close, conflicting groups = the
  top-left cells (-2,3) (-2,4) (-1,4) (0,4) (-2,5); 135-sym UNSAT. Geometry behind it (131): the
  top-left region (7 cells) must hold the lever's whole sequence: P3's level (only from (-1,4), or
  from (-2,3) which is also the L2/L1 feeder: a conductor there powers L2), the L2+L1 pulse (only
  from (-2,3): L1's quasi cell is L2's cell), D1's three shots (both D1 cells are fed only from
  (0,4): directly at (1,4), by quasi-connectivity at (1,3)), and possibly DE's window/pulses
  ((0,5); (2,5) would also power D2). Any observer at (0,4) must watch (-1,4), which is P3's only
  clean feeder; the lever can only sit outside the box next to (-2,3..5)/(-1,6)/(0,6). 
- The 102 window trick carries over (DE held for a window, D1's lifted shot from a watcher of DE's
  carrier), but its carrier would have to sit at (0,5) and the watcher at (0,4) must face west to
  feed D1 (so it watches (-1,4), not the carrier): same cell conflict as above.
- All four plans UNSAT under satwire (time warp 12/12, full palette block,dust,rep,obs,torch,note,
  bulb,cmp, lever anywhere around the box, front fix): 131-sym and 131-oc core = P3 (-1,3) on close
  (groups: top-left cells (-2,3) (-2,4) (-1,4) (0,4) (-2,5)); 135-sym core = UB (2,-2) + M's cell
  (1,-1) on close, 135-oc core = L0 (-2,0) + P3 (-1,-1) on close (groups: the top-left/left cells).
  Logs: logs/sat-f131-{sym,oc}.log, logs/sat-f135-{sym,oc}.log.
- Mechanism changes considered for 131 (not validated): T aside to the RIGHT (SW+(3,3), TR right
  via SW(4,2)) frees the top-left of P3 and lets the big right region feed D1, DE and P3'; but then
  the left stack (L2+L1 from (-2,3), L0 from the bottom-left) needs single pulses, and the only odd
  links left (level pistons) are fed from the right region and watched from nowhere on the left.

### 131 in the 7-wide box (x -2..4, y -2..5 = 7x8 = 56b; the coordinator checked the mechanism stays inside)

- satwire on both plans with --box=-2,-2,0:4,5,0 (28 free cells, 94 lever candidates, time warp
  12/12, full palette): UNSAT, core P3 (-1,3) on close, groups (-2,3) (-2,4) (-1,4) (0,4) (-2,5)
  (logs/sat-f131-{sym,oc}-7w.log). Also UNSAT in both 8-wide placements (x -2..5 and x -3..4).
- Regions in 7x8: TL 7 cells, TR 8 + ring (3,2), BR 5 + ring (2,-1), BL 5 + ring (0,-1) (the x=5
  column that joined TR and BR in 8 wide is gone). Pair feeds that work: (-2,3) powers L2 and
  quasi-powers L1 (both left toggles at once); (4,2) powers R1 and quasi-powers R0 (so BR has no
  duty). Odd links across regions: none. P3 (the only level piston) touches only TL cells; DE (TL
  or TR fed) is a pulse/window piston (even); L0 can be fed only from BL's (-2,-1) (or a lever on it,
  which makes it a level), and BL sees no piston driven from elsewhere (UM is fed from BL or BR,
  BR sees nothing either). So BL needs the lever, while TL's P3 level, L2/L1, D1's three shots
  ((0,4) is D1's only feed in both positions) and DE need the lever's timing too.
- Inside TL: P3's level can only come from (-2,3) (also L2's feed: a conductor there powers L2,
  so L2/L1 would be held), or from (-1,4)/its quasi neighbors: a wall torch at (-1,4) on (-2,4)
  (lever at (-3,4) on (-2,4) gives P3 at t2 on both edges, too early on open: T must already be in
  (1,3)); an observer at (0,4) for D1 must face west and watches (-1,4) (the torch: one pulse per
  edge, D1 needs three).
- Ideas not tried: BL exit DOWN with a LEVEL puller U0+ (0,-2) extended at rest, held by a wall
  torch at (-1,-2) on (-2,-2) and a lever at (-3,-2) on (-2,-2) (BL moves at ~t2 on both edges; its
  motion BUD-updates UM); or the 118 stopper idea applied to 131's D1/P3 coupling.
- Mechanism change TRIED and VALIDATED (f131/f131L.txt + f131L.ideal): ML and TL pulled by LEVEL
  pistons SE+(-2,1), SE+(-2,2) (extended at rest, heads in the ring at rest only) instead of pull
  pulses; ONE conductor at (-2,3) holds L2+ (direct), L1+ (quasi, through L2+'s cell) and P3+
  (direct). Symmetric schedule, WORKS at 32 positions x 2 cycles (open 39 / close 39), switch time
  5..9 ticks after D1's first shot. The top-left's duties drop to: that one symmetric level, D1's
  three shots (0,4), DE's two pulses (0,5).
- satwire on f131L in 7x8: still UNSAT (31.8 s), core L2+ (-2,2) on close, groups (-2,3) (-2,4)
  (-1,4) (0,4) (logs/sat-f131L-7w.log). Why: (-2,3) must be a STRONGLY powered conductor that is on
  at rest (a lever on it powers it while ON: inverted), so it needs dust on (-2,4) fed by a torch
  ((-2,5) on (-1,5), or (-1,4) on (0,4)), plus 5..9 ticks of delay, plus D1's feeder at (0,4) and
  DE's at (0,5): the 4-cell core. DE fed from the right instead ((2,5): a repeater facing east on
  D2) moves the problem to TR, which has no incoming link.
- Hand-wiring f131L's top-left (7 cells), where it jams: D1's three shots (t0 at (1,4), t18 lifted
  at (1,3), t30 at (1,4)) all come from (0,4) (D1's only feed in both positions); an observer there
  must face west and watch (-1,4), so (-1,4) must change three times per edge (on, off, on) without
  powering P3 below it: only a weakly activated note block works (a wall torch at (-1,5) or a
  strongly powered conductor at (-2,4) next to it). DE's two pulses come from (0,5): an observer
  there watches (-1,5) (the same torch: DE and D1 would fire together), a repeater there reads
  (-1,5) too. X (-2,3) must be strong and on at rest: dust on (-2,4) fed by a torch, 5..9 ticks
  after D1's first shot. Seven cells cannot hold all of: X's dust + torch + delay, (-1,4)'s 3-change
  source, (0,4)'s observer, DE's feed and the lever's attachment.
- The window variant (DE held 12..24, a watcher of its carrier for D1's lifted shot) needs the
  carrier at (0,5) and a watcher facing up at (0,4), which then cannot feed D1 at (1,4) (s1, s6).
- Bottom-left in 7x8: L0's park cell (-1,0) is filled only by L0 itself; 131 has no bottom
  elevator; UM is fed from BL or BR and BR sees R0 only if R0 is fed from TR (R0 quasi from (4,2)).
  Making R0/R1 LEVEL pistons (SW+(4,0), SW+(4,1), extended at rest like L1+/L2+) would give BR an odd
  link (a watcher at (4,-1) sees R0+ move once per edge) to fire UM; UM's two motions then reach BL
  ((0,-2)) as a pair 2-4 gt apart, which a merger (observer + delay-2 repeater) could turn into L0's
  single pulse, if it fits in BL's 5 cells. Not tried.
