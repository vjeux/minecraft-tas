# Track wire56: a flat, slime-free, FULL seamless + hallway door in a 7x8 box (56b)

Goal (coordinator, 2026-10-18 22:37Z): 1-wide (flat), slime-free, FULL seamless + hallway-compatible
door in a 7 wide x 8 tall box = volume 56 (record 72b: e89b, 8x9x1, explore/flat-seam). Lead: agent
families' v1 catalog (6b2d389) entry 7x8-028 (= v2 7x8-013), which `door topology` calls wirable
only with MEMORY links (waxed copper bulb T flip-flop + comparator).

## 1. The mechanism: 7x8-028 (mech/m028.txt), box x=-2..4, y=-2..5
`SE(-2,0) SE(-2,1) SD(0,4) SU(1,-2) SD(1,4) SD(1,5) SU(2,-2) SW(3,3) SW(4,1) SW(4,2)`
- Left pair (-2,0)/(-2,1) pull BL/ML into (-1,0)/(-1,1); TL pulled up into (0,3) by (0,4); BM and BR
  pulled down by (1,-2)/(2,-2); MR/TR pulled right by (4,1)/(4,2).
- Core: P4 = (1,4) (sticky, down), P5 = (1,5) (sticky, down, the elevator), PT = (3,3) (sticky, west,
  ring corner). Sym train (13 pulses, the same on both edges): s1 P4, s2 PT, s3 P5, s4 P4 (lifted to
  (1,3)), s5 P5, s6 P4, then the 7 toggles.
  - open: s1 pulls T up into (1,3); s2 pulls T aside into (2,3); s3 pushes P4 down into (1,3) (spit);
    s4 (lifted P4) pulls C up into (1,2); s5 pulls P4 back up to (1,4); s6 pulls C into (1,3).
  - close: s1 pushes C from (1,3) to (1,2); s2 pushes T from (2,3) to (1,3); s3 pushes P4, T, C down
    one (T and C home); s4 (lifted P4) pushes T and C one further down (T (1,1), C (1,0)); s5 pulls
    P4 back up; s6 is a no-op (reach-through pull of an empty cell); BM's push (1,-2) then pushes BM,
    C, T up one: all home. So on close BM's toggle must come after s4.
- `door families --check mech/m028.txt --verify --extra-offsets 28`: oc and sym both WORK with ideal
  inputs at 32 positions (mech/ideal/m028-{oc,sym}.{txt,ideal}).

## 2. Topology (door topology, memory links)
Regions of free cells (stone hops merged): K0 bottom-left (5 cells: feeds (-2,0), (1,-2)), K1
top-left (8: feeds (-2,0) by QC, (-2,1), (0,4), (1,5)), K2 top-right (7: feeds (1,4) at rest AND
lifted (via (2,4), QC through (1,4)), (1,5), (3,3), (4,1) (QC), (4,2)), K3 bottom-right (5: feeds
(2,-2), (4,1)). Links: (-2,0) watched from (-2,-1) (K1 -> K0, 1 change), (4,1) watched from (4,0)
(K2 -> K3, 1 change), P5 watched from (0,5) / (2,5) (K1 <-> K2, 2 changes per edge: even).

Why the lever must be in K2 (not K1, which topology prints): K2 must fire s1 (P4) and s2 (PT)
BEFORE P5 first moves (s3), and P5 is the only thing K2 can see of K1. K1 cannot fire P4 alone: a
source at (0,5) powering P5 also quasi-powers P4, but it updates P5 first (diode/observer output
updates the front block, then its neighbours), so P5 moves first and P4's queued event is dropped
(its cell became a moving block). So K2 holds the lever and runs the core + right side; K1 (top-left)
needs ONE pulse per edge for (0,4) and the left pair from P5's two moves (even): the memory link.

## 3. The memory link: what the bulb + comparator really gives
- `designs/gadgets/memory-window.drive`: bulb B + comparator C turn two pulses into ONE window.
  A window OPENS a pull-type piston (extend at the rise, sticky pull at the fall) but cannot CLOSE
  it: the window pushes the block back, then the fall is a normal sticky retract (the pushed block
  has landed) and pulls it out again. A pull-type piston needs a short (<= 2 gt, spit) pulse on close.
- `designs/gadgets/memory-link.drive`: the correct even->odd link = two flip-flops: B1 -> C1
  (window) -> B2 (toggles once per edge) -> C2 -> observer watching C2 = exactly one 2 gt pulse
  per edge, 6 gt after the first input. 7 cells (with 2 comparator floors) in a 4-cell horizontal
  line (B1 C1 B2 C2): it does not fit K1 (rows of 2-3 cells, (0,5) is the only cell watching P5).

## 4. Odd counts without memory: observer busy windows (t1/t2, WORKS with ideal K2)
An observer ignores a change while a tick is pending or while it is powered, so the number of
pulses an observer WATCHING A PISTON gives depends on the timing (each 2 gt piston pulse = changes
at extend e (block-event), retract e+2, landing e+4 (block-entity phase); seen = e and e+4):
- K1: P5's two pulses exactly 6 gt apart: O(0,5) sees e, e+4 (landing), hides the second extend
  (powered), sees the second retract (it turned off in the scheduled phase just before): 3 raw
  pulses, 4 gt apart = ODD. Relayed by observers to PTL (O(-1,4) facing west) and to the left pair
  (O(-2,2) facing up: direct into (-2,1), QC into (-2,0)); a waxed bulb at (-2,3) is the relay cell
  above O(-2,2) (an observer there would QC-power (-2,1) from (-2,3): first attempt fired it twice).
  Each pull-type piston takes 3 pulses 4 gt apart (net one move; the 2nd re-extends 1 gt late
  when its base lands).
- K0: O(-1,-1) watching BL's park cell (-1,0) (not PBL's base) sees exactly 3 of its changes on
  both edges for three PBL pulses 4 gt apart; note block (-1,-2) + O(0,-2) into PBM.
- K3: O(4,0) watching PMR's base sees 5 raw pulses for three PMR pulses 6 (or 7) gt apart; note
  block (4,-1) + O(3,-1) into (2,-1) (QC of PBR).
- test/t1.txt (mechanism + those 13 wiring blocks) + test/t1.ideal (K2 emulated by redstone
  blocks at z=1): WORKS at 32 positions x 3 cycles, open/close 52.
- t2.ideal sweeps (K2 windows, with s3 = 12): s1 in s3-11..s3-7, s2 in s3-5..s3-3 (and >= s1+3),
  s4 (lifted P4) in s3+2..s3+3, s5 = s3+6 EXACTLY with a 2 gt pulse (K1 and K0 parity), s6 in
  s3+10..s3+12; PTR+PMR three pulses with gaps 6 or 7 (K3 parity), any start.

## Lemma (7x8-028 = v2 7x8-013, and 029 = v2 7x8-014): no static wiring in x=-2..4, y=-2..5
Cells: P4 (1,4), P5 (1,5), PT (3,3), PTR (4,2), PMR (4,1); K2 = (2,4) (3,4) (4,4) (2,5) (3,5)
(4,5) (4,3).
1. P4 at rest and lifted (1,3) is fed only from (2,4): its other feeds (0,5)/(2,5) (QC via (1,5))
   always power P5 too and P5 is updated first (P4's event is dropped); (1,6) is outside the box;
   lifted P4's QC cell (1,4) is empty between s3 and s5, its other neighbours are ring cells.
2. (2,4) cannot be a conductor (it QC-powers PT through (3,4)), dust/torches have no support
   there ((2,3) is swept), so it is an observer facing east or a delay-1 repeater/comparator with
   input east (a longer pulse re-pulls C on close): it reads only (3,4).
3. (3,4) is PT's quasi cell and sits on PT: no conductor, no dust/repeater (an extended PT has no
   full top face), no torch (powers PT): only an observer facing east, watching (4,4).
4. PMR is fed from K3 only through (4,0), and K3 sees nothing but PMR, so K2 must fire PMR: only
   by emitting down into (4,2) from (4,3) (QC), which also fires PTR. (4,3) touches PT, so it is no
   conductor: an observer facing up watching (4,4).
5. So P4 = changes of (4,4) + 4 and PTR+PMR = the same changes + 2 (both observers see the same
   updates of the same cell). P4 must get exactly 3 pulses (s1, s4, s6; extra pulses push C or T
   out), so PMR gets 3 pulses with gaps s4-s1 in 9..14 and s6-s4 in 7..10 (measured windows,
   t2.ideal). K3's watcher (4,0) counts 2 pulses for an isolated PMR pulse and 3 for a pair 5..7 gt
   apart, 4 for 8..10: odd only if s6-s4 = 7, an odd gap.
6. Every K2 signal is even-timed: the lever flips between ticks, observers/repeaters/comparators
   /torches delay 2k gt, and the pistons K2 can watch are fired by those signals (events and
   landings at even ticks). Odd ticks need a piston the lever powers directly (k1) or leaves/
   scaffolding next to a moving log. Lever outside the box (coordinator's idea 1): the cells of
   points 1-4 stay mandatory; a lever powering PTR/PMR/PT/P5/P4 directly makes that piston
   level-driven (PTR's head would sit in ring cell (3,2) when open; PT extended at rest is 029,
   whose P4/PTR chain is the same). Leaves (idea 2): no log moves in 028/029 (door blocks are the
   only moving plain blocks; a log door block T parking at (2,3) is next to (2,4) only, which point
   1 reserves for P4's observer). Hence no 7x8 wiring for 013/014 under static redstone.

## 5. Why K2 cannot be wired: 7x8-028 is not wirable in 7x8 (18:45Z)
K2 (top-right, 7 cells (2,5) (3,5) (4,5) (2,4) (3,4) (4,4) (4,3)) holds the lever and must fire
P4 three times (s1, s4, s6), PT once, P5 twice, PTR+PMR (K3's only input) an odd number of times.
- P4 (rest AND lifted) is fed only from (2,4) (the other path, (2,5)/(0,5) via P4's QC cell (1,5),
  always moves P5 first). (2,4) must emit west: an observer facing east watching (3,4) (a conductor
  at (2,4) works the same). (3,4) is PT's quasi cell: anything emitting INTO (3,4) fires PT, so
  (3,4) must be an observer facing east (facing up emits into PT, facing down gives P4 PT's double
  pulses). So P4's pulses = changes of (4,4) + 4.
- PTR (4,2) + PMR (QC) are fed only through (4,3) (a conductor there powers PT): an observer facing
  up, watching (4,4). So PTR/PMR's pulses = the same changes of (4,4) + 2: the same train as P4.
- P4's train: f2 in {s3+2, s3+3}, f3 in s3+10..s3+12, so f3 - f2 in 7..10; K3 needs that last gap in
  5..7 (after a first gap >= 8), i.e. f3 - f2 = 7 exactly, an ODD gap. Every K2 signal is even
  (observers, repeaters, torches, lamps, bulbs, comparators all delay by 2k gt from the lever's
  observer; odd ticks only come from piston landings / leaves / scaffolding, and K2 has no free
  piston or moving log). Even gaps 8 or 10 give K3 6 raw pulses: PBR moves an even number of times.
- K3 (5 cells + the ring stone (3,0)) cannot hold a pulse merger (repeater + floor + observer: the
  observer watching the repeater would sit in a ring cell or the floor, outside the plane's room)
  nor the 7-cell double flip-flop; a lamp or one bulb gives windows (fail on close).
- satwire agrees on the exact plans: families' m028 plans (P4 recorded correctly after the z=1
  fix) and t2's K2 plan restricted to the K2 box: UNSAT.
- 7x8-029 (= 028 with PT level-driven, extended at rest) has the same P4 / PTR chain through (4,4)
  plus a level feed for PT in the same cells: same contradiction. 038/039 have no symmetric train:
  in families' paths five pistons need different counts on the two edges ((0,4) 2 vs 3, (1,4) 3 vs
  2, (-1,3) 1 vs 4, (1,-2) 1 vs 4, (4,1) 1 vs 4), i.e. edge-state memory (the 7-cell double
  flip-flop) in 5-8 cell regions: not attempted.
- Side finding: an immovable block at the ring corner (-1,3) (obsidian) makes PT's push a no-op
  whenever the top ring row is T, C, TL (the open state), so extra PT pulses become harmless before
  s1 on both edges and after TL parks: PT does not need to be the bottleneck.

## 6. 7x8-038 and 7x8-039 (memory-only, oc): not wirable in the plane (19:55Z)
- Gadget `designs/gadgets/qc-shared.drive` (PASS): a lower piston whose only feeds are quasi-
  connected through the cell of the piston above it cannot fire alone: every such source powers
  the upper piston directly and updates it first (observer output: front block, then its
  neighbours), the upper piston's push carries the lower one and its event is dropped.
- 038: P4 (1,4) sits between the pistons (0,4) and (2,4); its only feeds are (0,5)/(2,5) (QC via
  P5's cell (1,5)), so by qc-shared P4 never fires at rest without P5 carrying it. Both of 038's
  paths fire P4 at rest with P5 idle (open step 2 `PSD(1,4)` pulls T up). Dead.
- 039: P4' (1,-2) (center elevator below) has pistons on all three free sides ((0,-2), (2,-2),
  P5' (1,-3)) and its quasi cell (1,-1) is the ring cell in front of it (neighbours: ring and door
  cells): no feed at all, and the open path fires it first. Dead.
- Same check on the permissive 9-piston entries: 001 (P4' flanked by (0,-2)/(2,-2)) and 002 (P4
  flanked by (0,4)/(2,4)) die the same way; 000, 003, 004 keep a free cell beside the elevator's
  lower piston.
- Tool idea for families/topology: model "shared QC" (a piston fed only through another piston's
  cell fires only together with it, and second) and "no feed" pistons, which the macro model and
  the region analysis currently accept.
- 000/003/004 (the remaining 7x8 candidates) need the macro model's cheapest close paths of 45-46
  events (PT fired up to 13 times, the left pistons 7 times per edge) next to 21-22 to open: an
  edge-dependent sequencer of that size does not fit 26 free cells; not attempted.
- Lemma note: an obsidian block at (-1,3) makes PT pulses harmless at rest and when open (row 3
  empty, or full and blocked), so (4,3) could be a conductor; it is then powered only by (4,4)
  emitting down, and P4 still reads (4,4) through (3,4): the same event train, same odd-gap wall.

## Verdict (20:00Z)
No flat FULL-seamless slime-free door in 7x8 from families' catalog: 013/014 (lemma above), 038,
039, 001, 002 (unfeedable elevator piston, gadget qc-shared), 000/003/004 (45-event close trains).
What carries over to other boxes: odd pulse counts without memory from observer busy windows
(section 4), the 2-flip-flop memory link (7 cells), and the qc-shared rule.
