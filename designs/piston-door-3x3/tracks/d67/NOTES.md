# Track d67: feeding the pistons of c70b's 6x7 (42) mechanisms (clk-misc session, 2026-10-20)

Brief (coordinator, 20:40 / 20:57 PT): c70b's best 6x7, explore/c70b 254d27d79
`smaller/mm/6x7-in2/mm01-oc` (11 sticky + 1 normal, statics (3,0..2); WORKS at 32 positions, open 129 /
close 39) has two wiring problems: (a) one 2-gt pulse to the piston carried into (0,1) at open t72 (plan
input in the hallway), (b) SU(1,-2) has no feed. Find feeds (door blocks as conductors, quasi-connectivity,
carried observers, block-update tricks), prove them in redsim, hand them to pockets3 / c70b.

Working copies: `in2/mm01-oc.{txt,ideal}` (c70b's, design path rewritten). Tool: `door ideal-rs --planes
LO..HI` draws the door plane after every tick of a window (open and close): what touches a carried piston
when it fires.

## The mechanism, as the planes show it (pistons by rest cell)
a SE(-2,-2), b SU(-1,-2) (pushed by a to (0,-2) at open t37, back at close t7), c SU(1,-2), d SU(2,-2),
e SU(-1,-1) (rides the left column (-1,-1..0)), f SE(-2,1), g SE(-1,2) (rides the left column and row 1:
(-1,1) / (-1,0) / (-1,2) / (0,1) / corner (-1,3) when open), h NE(-2,3) (fires once, open t109),
i SD(-1,4), j SD(0,4), k SD(2,4).
Cells never occupied during a cycle (the only places static wiring can stand): (-2,-1), (-2,0), (-2,2),
(-2,4), (1,4), (3,4), (3,3), (3,-2), (3,-1). Regions: {(-2,-1),(-2,0)}, {(3,-2),(3,-1)}, {(3,3),(3,4)}, and
the single cells (-2,2) (between f and h), (-2,4) (between h and i), (1,4) (between j and k).

## (b) c = SU(1,-2): no feed inside the 6x7 box (proof by cells)
c fires at open t127 (pull BM down) and close t37 (push it back). Direct neighbours: (0,-2) (b's waypoint,
swept open t37 -> close t11: no static block can stand there), d (2,-2), (1,-3) outside the box, (1,-1)
its front. Quasi cell (1,-1): neighbours (0,-1), (2,-1), (1,0), all frame / door cells. The only conductor
there a free cell touches is BR at (2,-1) (from the corner (3,-1)), and only at close:
- a source at (3,-1) powering BR also QC-powers d (its above cell is (2,-1)) and updates it; d's push moves
  BR (moveBlocks makes (2,-1) a moving piston) BEFORE d's base update reaches c, so c sees no power;
- at open BR is still at (2,0) when c must fire; pulling it down first makes d re-fire on the same source.
- the lever on c's front face (1,-2,1) (legal, below the opening's columns): TESTED (`in2/mm01-oc-cl`,
  c extended at rest, the lever drives it on both edges): the lever POPS at the open flip. An attachment on
  a piston's SIDE face pops: the extended base's side faces are 16 x 12 (not full), and a retracting base
  is a moving piston with that shape (same for a repeater standing on f's top face, `in2/mm01-oc-q7`'s
  first try). CORRECTION (21:35 PT): on the piston's BACK face (the face opposite its head) an attachment
  survives extension and retraction (that face stays full): `in2/mm01-oc-cb`, the lever at (1,-3) hanging
  from c's bottom face (face=ceiling), c lever-held (rests extended, lever ON; its two actions moved from
  open t127 / close t37 to t0 of each flip), WORKS in redsim at 32 positions x 2 cycles and in the game
  (`door ideal`, 4 x 2). The lever is outside the box, which is fine: the volume excludes the lever
  (properties.rs drops the lever cell). So the lever can hang on the back face of any piston that is never
  moved (c78l's lever-behind-held-piston recipe, e88's lever on top of a down-facing puller), and that
  piston needs no feed. Catch: the rest of the door must be woken from the lever, and (1,-3)'s neighbours
  (0,-3), (2,-3) are outside a 6x7 or 7x7 box; c's and BM's moves at t0 touch no free cell either. In this
  box the lever there wakes nothing else (in a 6x8 the row y = -3 would hold the wiring next to it).

## (a) g carried into (0,1), 2-gt pulse at open t69..t73: no feed inside the 6x7 box
g lands at (0,1) at t69 (f's spit) and must pull MR (2,1) -> (1,1); f pulls it back at t79. Around it:
(0,0) empty, (-1,1) empty, (1,1) its front, (0,2) TL; quasi cell (0,2) with neighbours ML (-1,2), TM
(1,2), (0,3) (empty ceiling cell). Power can reach g only from (0,3), (-1,1), (0,0), (1,1) (frame / door
cells) or through ML (strongly powered from (-2,2), (-1,3) or (-1,1)) or TM (from (1,3), (1,1), (2,2)).
The corner (-1,3) is ML's / g's waypoint (t49-63, t91-108, g from t115); the only free cell is (-2,2), and
it is sealed: f below, h above, (-3,2) outside the box. Only full blocks survive there (anything standing
on f pops when f extends, tested), and no full block there can emit east without an input from (-3,2):
an observer facing west watches (-3,2); facing up / down it outputs into h / f (an f -> h or h -> f relay,
nothing more); a repeater / comparator facing east takes its input from (-3,2).
What does work physically: a carried piston FIRES ON ARRIVAL when it lands on a powered spot (onPlace
re-checks its power; no extra update needed), and a strongly powered frame block QC-powers a piston whose
above cell it touches. So with ONE more column (x = -3, a 7x7 = 49), an observer at (-2,2) facing west
(watching (-3,2)) firing into ML at g's landing tick would fire g at (0,1) through QC.

## Also unfeedable in 6x7 (connect is optimistic about output directions)
i = SD(-1,4) (open t61, close t25): neighbours (-2,4) (sealed single cell: h below, i, outside), j, (-1,5)
outside; its quasi cell (-1,5) is outside. A full block at (-2,4) cannot emit east without (-3,4) (as
above); a torch on h pops at h's retraction. h is fed only from the sealed (-2,2) / (-2,4).
Ways out: the lever on a stone at (-2,4) would give i and h the lever's LEVEL (both edges at t0); or the
7th column (x = -3) feeds f, h, i, (-2,2) and (-2,4) like the plan's own (-3, y) inputs; a row below
(y = -3, 6x8 = 48) feeds a, b, c, d like the plan's (x, -3) inputs. The plan was written for those
cells: every non-hallway input of the plan sits at x = -3 or y = -3.

## pockets3's satwire on the 7x7 variant (21:26 PT)
`in2/mm01-oc-q7` (7x7: x -3..3) with every input left to satwire (solid, FULL frame, forbid-ring, update /
exact order, --rec-ticks 200, wide palette): UNSAT at once, core = b fired at (0,-2) (open t43, close t1),
which the plan feeds from (0,-3). In 7x7 its only feeds are its quasi cell (0,-1)'s neighbours: (-1,-1)
(empty then, e's rest cell), (1,-1) and (0,0) (door blocks with no powered neighbour). So the family needs
the row y = -3 for b (and c, unless c is lever-held) AND the column x = -3 for g / f / h / i: 7x8 = 56.

## m69g-12 (9x6 = 54, c70b v9x6/mm01-oc): W' (3,-1) is timing-sealed
W' = SU(3,-1) (the elevator under SW(3,0)) pulses at open t25 / t43 / t67, never on close; the plan feeds
it from (3,-2), below the box. Its in-box feeds: the static (2,-1) (no component can power it: its other
neighbours are the static (1,-1), BR (2,0) and outside) and the pocket {(4,-1), (5,-1), (4,0)}, whose only
neighbours are W' itself and the pistons SW(3,0), SW(4,1), SW(5,0). None of them moves before t25, and the
floor statics (0..2,-1) block any signal from the left (strong power never hops stone to stone; a torch
would need a free cell beside a static). So nothing in the pocket can know t25: satwire's UNSAT core is
physical. The right side needs a free cell that watches something moving before t25, or W''s first lift
moved after a right-side event.

## `door connect` fixed (explore/d67 320d8a1a4; c70b merged it, 200863dc8)
connect is c70b's mechmut ranking filter (a necessary condition: NOT wirable = no order of events can be
wired). It had false negatives on REAL doors: in their own boxes (`door audit --sections mech`) it said NOT
wirable for e78s-56b-43 (the solid 7x8 record), e78-56b-40, e78st-56b-37, t7b-nb2, pockets-63b-43 and -44.
All six hold the top trio with e88's lever OUTSIDE the box on SD+(1,4)'s back face (QC holds SD+(0,4) and
SD+(2,4)), and connect only put the lever in a free-cell region inside the box. Now it also tries the lever
on the back face of every never-moved piston and on the outer face of every static, outside the box: all
16 real flat doors tested (`real-flat16.tsv`) are WIRABLE in their own boxes. Second change, stricter:
SEALED cells (a free cell on its own with nothing beside or below it that stays put) hold an observer only,
so they feed the cell opposite the change they watched and read no powered block. For 6x7-in2/mm01, never
fed goes from 1 piston to 4-6, which agrees with satwire. `door connect` also reads mechmut TSVs now (a
`mechanism` column: each variant in its own box) to re-screen c70b's catalogs.

## w86's 7x7 impossibility argument (explore/w86 162a73b32 "7x7 (49)" 1-4): the hole
w86: every 7x7 has a piston P in a middle frame cell M when the center takes its first step; P's edge block
later fills M through the outer puller behind M; P escapes along the frame (a sticky helper Q on one side,
its lane on the other); so at P's firing time its 4 neighbours are all taken, and P is unfeedable. Argued
for M = F1 (the floor middle), "same by rotation for C1/L1/R1".
The rotation does not hold, because quasi-connectivity always looks UP and plain blocks conduct:
- F1: P faces up, its front is BM (ignored), its quasi cell IS that front cell, whose neighbours are door
  cells: no feed (w86 is right there).
- L1 / R1: P faces east / west. The frame cell above it (L2 / R2) is both a side neighbour and its quasi
  cell, and its outer neighbour is a MARGIN cell ((-2,2) / (4,2)). If L2 holds a plain block (taken!), a
  component in (-2,2) strongly powers it and P is directly powered; if L2 is empty (P's lane), the
  component outputs into L2, P is quasi-powered and the output update reaches it (L2's neighbour).
  A "taken" neighbour is not a blocked feed when it is a plain block with a free neighbour.
- C1: P faces down, its quasi cell is the margin cell above (1,4) (w86's outer puller: quasi through it
  couples with that puller); its side neighbours C0 / C2 can carry power from (0,4) / (2,4) when they hold
  plain blocks.
Witness in a 7x7 box: `in2/mm01-oc-q7b` (q7 = c70b's 6x7-in2 + the column x = -3): g fires at L1 (-1,1) at
open t43 with its 4 neighbours taken (f (-2,1), e (-1,0), a stone in L2 (-1,2), its front), and its own
plan input is gone. It is fed only by the observer at (-2,2) watching (-3,2) (a block appears there at
a = 36..43; at t69 it changes again for g at (0,1)). WORKS: redsim 32 positions x 2 cycles, the game 4 x 2.
(q7 as a whole still needs the row y = -3 for the bottom row: pockets3's core b at (0,-2); the witness
is about the feed, not a wired 7x7.)
Other gaps: (2) assumes BM fills F1; F1 can be filled by a block slid along the floor from F0 / F2, which
frees (1,-2) right under P (a direct feed cell). The flat rule lets a lever hang on any block's front /
back face (rules.md), so any static is a lever-powered hub (levels at t0).
So a 7x7 search should put the middle-frame piston in L1 / R1, keeping the margin cell beside L2 / R2 free
(mechmut `--reserve`), e.g. w1's class turned in-plane (P pushing the middle row) and repaired.

## Feed tables (`door feeds`, explore/d67 2fda8c604 .. 22a27a315)
Per firing of an ideal plan (each redstone block it places): the pistons it powers and every flat feed
option in cells free ALL cycle inside the box (cells only the plan writes count as free): a free
neighbour (not the front), QC into the cell above, a conductor N powered from a free M, a fixed observer
of the design. Flags: ! = an observer / repeater there would watch or take input from outside the box;
? = from a block that never changes (dust / torch at best); @ / ~ = an observer's watched cell changes
the tick before the firing / only at other ticks. Conflicts: one cell that is the only clean option of
firings needing different targets. Library: `piston_door::feeds::analyze` -> FeedReport.
- q7b (7x7, in2/q7b-feeds.txt): 39 of 42 firings fed; unfed: b at (0,-2) (open t42, close t0) and
  SU(1,-2) at open t126; conflict: (-2,0) is the only clean feed of SE(-2,1) (up) and SU(-1,0) (east).
  SU(1,-2)'s close feed (QC through BR at (2,-1) from (3,-1)) is not buildable: the right pocket
  (3,-2),(3,-1) can only hold components watching / fed from x = 4 (outside), torches stay lit, dust has
  no source. Delaying the open pulse to use BR would also re-fire SU(2,-2) (BR's cell is its QC cell).
- w86's q7d (BM puller sharing a 6 gt pulse with SU(2,-2)): same pocket problem; and BR leaving F2
  during the wiggle cuts SU(1,-2)'s QC (its extend event re-checks power: dropped or a 0-1 gt pulse).
- c70b's u0-7x7 (explore/c70b 1d105a221, u0-7x7-feeds.txt): no unfed firing, 7 weak ones on the box
  edges (SW(3,-1) x2, SE(-1,1), SD(-1,3) x3, SE(-1,-1)); conflict: the sealed (0,-2) is the only clean
  feed of both SU(1,-2) and SU(-1,-2) (and its 2 gt delay orders them wrongly).

## Wiring u0-7x7 (u0/: u0-l2 .. u0-l6)
- Islands: u0's (and u2's) 16 free cells are 5 regions with no electrical path between them (pistons do
  not conduct; no block touches two regions): A2 {(-2,-2),(-2,-1)}, B4 {(-2,2),(-2,3),(-2,4),(-1,4)},
  C1 {(0,-2)}, D5 {(2,4),(3,3),(3,4),(4,3),(4,4)}, E4 {(3,-2),(4,-2),(4,-1),(4,0)}. A region can only be
  triggered by observing the pistons next to it, or by the lever if it touches it. D5 sees only SD(1,4)
  and SW(4,2) ((3,2) only changes by SW(4,2)), which only D5 feeds; E4 sees SU(2,-2), SW(3,-1) (E4-fed)
  and SW(4,1). With the lever at the top (B4 sees SD(0,4)), D5 and E4 never start: u0 as given has no
  static wiring.
- The lever: on SD(0,4)'s top (0,5) it powers SD(0,4) and QC-powers SD(1,4) (the lever touches (1,5)).
  Anywhere else near SD(0,4) also powers SD(-1,3) (anything emitting into (-1,4) QC-powers it).
  u0-l2: SD(1,4) extended at rest, held by the lever (its open reach-pull and close push happen at the
  flip): WORKS redsim 32 x 2 (57/45). D5 now sees the lever through SD(1,4).
- u0-l3: SW(4,1)/(4,2) work at any tick, both edges (64/64 early pairs at 32 positions).
- u0-l4: D5 wired: observers (2,4)W (watching SD(1,4)) (3,4)W (4,4)W (4,3)U (into (4,2): SW(4,2)
  directly, SW(4,1) by QC); an observer watching a 2 gt piston action gives one pulse (the later changes
  fall inside its pending tick). One pulse per edge, SW(4,x) at ~t9. WORKS redsim 32 x 2, game 4 x 2.
- E4 is next and does not fit u0's timing: it sees only SW(4,1) (the same on both edges) and has 3
  spare cells (no memory), so SU(2,-2) and SW(3,-1) must act the same on both edges after SW(4,1);
  u0-l5's windows: SU(2,-2) #1 open t1..10 (before SW(4,1)), #2 open >= 25, extra close pulse harmless
  only at t25..26 (u0-l6); SW(3,-1) open t23..28, close t35..41 (disjoint). Needs a symmetric train
  (pext's interleave) or a mechanism variant (c70b) with those constraints.

## Sculk on u0 (u0/u0-s0 .. u0-s2; `door feeds --sculk`)
Timing (26.3 `VibrationSystem`, as redsim's sculk.rs ports it; game-checked gadgets sculk-hold /
sculk-delay-line): a lever flip and every piston extend / retract are vibrations (BLOCK_ACTIVATE 10 /
BLOCK_DEACTIVATE 9; observers, dust, repeaters and ideal inputs make none). An idle sensor (INACTIVE,
nothing in flight) commits to the first tick T with an event within its radius (8, calibrated 16;
that tick's closest event), activates at T + max(1, floor(d)) in the block-entity phase (adjacent
pistons move from T+floor(d)+1), stays ACTIVE 30 gt (calibrated 10), COOLDOWN 10, and hears nothing
from T until INACTIVE again. Power is weak on every side (calibrated: not its back), strong only
into the block below. An observer watching a sensor pulses 3 times per activation (wake, cooldown,
inactive: +0, +10/30, +20/40, each +2).
- In a 7x7 box every cell is within 8 of the lever (0,5) or of the t1 moves of SD(0,4) / SD(1,4):
  every sensor wakes at t1..t8 on BOTH edges (`--sculk` on u0-s0: bottom right t6..t8), then re-arms
  at +20 (calibrated) / +40 and takes the next vibration. Sensor actions are early and the same on
  both edges, unless the mechanism's state differs or a calibrated sensor's back gets a 9 / 10 signal
  (u0's lever turns OFF at open: deactivate 9; ON at close: activate 10).
- A sensor gives a HOLD of 10 / 30 gt: a sticky piston pushes and pulls back (no spit); a reach-pull
  (front empty) still happens, at the hold's end. A NON-sticky piston pushes and leaves the block on
  any hold: u0's SW(3,-1) only push-spits, and as a normal piston (u0-s2) the door works with its
  pushes held 2..30 gt, but only starting open t23..28 / close t35..40: no sensor offers that (first
  wake t7, re-armed t27 / t47 + the next event).
- The bottom-right island's only observer spot that watches inside the box and outputs into a
  piston is (3,-2) facing east (into SU(2,-2), watching (4,-2)): a sensor at (4,-2) gives SU(2,-2)
  3 pulses per activation on both edges; u0 needs 2 at open, none at close (harmless only t25..26).
- (0,-2): a sensor there holds SU(-1,-2) and SU(1,-2) together; SU(1,-2) push-spits at close.
- Brute force (redsim, `ideal-rs --vary`, u0-s1: the real lever, the bottom-right inputs removed):
  0 of 1296 fillings of (3,-2) (4,-2) (4,-1) (4,0) with air / sensor / calibrated x4; 0 of 2058 with
  observers and dust added for SU(2,-2) alone (u0-s1a); 0 of 2058 for SW(3,-1) alone (u0-s1b).
So sculk cannot rescue u0 / u2's islands: they need late, edge-specific spits. A sculk 7x7 needs a
mechanism whose sculk-fed pistons act early (or at +20 / +40), do holds or non-sticky pushes, and
differ between edges only through the door's state.
