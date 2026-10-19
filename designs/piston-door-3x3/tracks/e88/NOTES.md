# Track e88: an 8x8 = 64 flat FULL-seamless door from e89b (2026-10-19)

## RESULT: t7b = 63 (7x9x1) solid, FULL seamless + hallway, 43 blocks (BEST.txt; t7a = 45)
t7b: the row -4 chain shortened (observer (0,-4) east = D support, stone (-1,-4), dust (-1,-3) into D;
P_M from (-1,-2) watching the dust), archive/e88-t7b-43b.txt, open/close 43/31.
`door verify --extra-offsets 28` WORKS (32 positions x 3 cycles, rotation-safe, 20 cycles, hallway
yes), open/close 47/35, archive/e88-t7a-45b.txt. Ties the 63 volume record (c79-pockets-50b) with 45
blocks instead of 50. How it works (x -2..4, y -4..4, door 0..2 x 0..2):
- Top: a lever ON at rest standing on the T puller (1,4) holds the three ceiling pullers (T direct,
  TL/TR by QC); the lever flip pulls the whole ceiling up at t1 (zero wiring).
- Walls: (-1,3) observer watching TL's park -> (-2,3) repeater -> (-2,2) observer up -> ML puller
  (direct) + BL puller (QC); mirrored on the right ((3,3), (4,3), (4,2)).
- Bottom right: (4,-1) observer watching BR's puller (two pulses) -> stone (4,-2) -> delay-2
  repeater (3,-2) (merger) -> observer (2,-2) east = A's pulse 0 at t15 (pulls BM to (1,-1) on open,
  pushes C up on close).
- Bottom: (2,-3) up watches (2,-2) -> note block (2,-4) -> (1,-4) east -> note block (0,-4) ->
  (-1,-4) east -> note block (-2,-4) -> (-2,-3) down -> (-1,-3) west = input of the delay-4
  repeater D (0,-3) (on (0,-4)) at t25; (-1,-2) down watches (-1,-3) and fires P_M (-1,-1) at t27
  (BM to (0,-1)). D holds B (1,-3) t33..t41 (A lifted to (1,-1)); the gate observer (0,-2) watches D
  and powers BM (parked at (0,-1) only when open) at t35 and t43, exactly when A lands at (1,-1)
  and back at (1,-2): pulses 1 and 2 pull C down to (1,0) then (1,-1). On close BM is not at (0,-1):
  no pulses 1/2, B's 8 gt lift puts A, BM, C back.
- Why it fits 7 wide: e89b needed 8 columns for the top-right block sent right; one lever on the T
  puller sends the whole top row up instead, and the 9th row (row -4) gives D its support and the
  bottom a horizontal channel from the right (cut) side to the left.

## The 8x8 attempt (t3*): see below; bridge finished t3k with a calibrated sculk sensor (64b, 52
blocks, sculk category, archive/bridge-e88-sculk-52b.txt on explore/bridge).

Brief: shave a row off flat-seam-e89b-49b (8x9 = 72, FULL + hallway) to get 8 x 8, flat, 1 wide,
frame 12/12 (all 12 frame cells hold the door block when open), no slime/honey.

## Why e89b cannot simply lose row -4
e89b's row -4 holds the lever (0,-4) (excluded from the volume) and its only crossing of column x=1
(lever -> (1,-4) -> (2,-4) -> stone (3,-4) -> dust (3,-3) under B'), which feeds the whole bottom
right: B' (8 gt pulse from the delay-4 repeater D (2,-3), which needs the floor (2,-4)), X (2,-1) via
the observer (2,-2) watching D, and the right-bottom puller. In 8 rows (y -3..4) column 1 is mechanism
from top to bottom (T puller, T park, door, gap, A', B'), row -3 cannot hold a repeater or dust
(no floor), and the bottom right can only watch A' or B' (both driven from the left).

## The new top: ONE lever holds the three ceiling pullers (t1-t3, works in redsim)
A lever ON at rest standing on TOP of the T puller (1,4) (face=floor at (1,5): outside the box, the
volume excludes the lever) powers the T puller directly and the pullers at (0,4) and (2,4) by
quasi-connectivity (the lever is a neighbour of (0,5) and (2,5)); its `updateNeighbours` reaches
(1,4)'s neighbours, so all three are updated. All three are extended at rest (heads in (0..2,3)),
retract at the open flip (TL T TR pulled up into the ceiling) and push back at the close flip. The
extended puller's top face stays full (DOWN_AABB), so the lever survives (checked in redsim).
- t1: only T up (TL TR sideways as in e89b); t2: lever on the TR puller (2,4) (holds TR directly,
  T by QC); t3: lever on the T puller, the whole top row up. t3 frame at open: ceiling TL T TR,
  walls ML BL / MR BR, floor BM (0,-1) C (1,-1), static stones (-1,2) (3,2) (2,-1): 12/12.
- Clean crossings: an observer next to a level-held puller (or its park cell) fires exactly once
  per edge: (-1,3) watching TL's park and (3,3) watching TR's park, both at t3.
- Walls wired (t3r, 6 blocks, pockets finds): (-1,3) observer E -> repeater (-2,3) -> observer
  (-2,2) up -> ML puller (-2,1) direct + BL puller (-2,0) by QC (updated when ML's puller moves);
  mirrored on the right ((3,3), (4,3), (4,2)).

## The bottom (still ideal): why it is hard
With T home at t1 on close, A''s first pull pulse (A' lifted to (1,-1)) is blocked by the T puller's
head (harmless), but its second pulse (A' back at (1,-2), BM sitting at (1,0)) pulls BM down: the
second pulse must happen on OPEN only. Gate: BM parked at (0,-1) exists only while open; an observer
(0,-2) facing down powers it, A' at (1,-2) takes it by QC on its landing tick.
- B' cannot get an 8 gt pulse in 8 rows (row -3 has no floor for a repeater). Shuttle: sticky piston
  (-2,-3) pushes the redstone block (-1,-3) into (0,-3) (powers B'), pulls it back later; the gate
  observer watches (0,-3), so it fires exactly when A' lands back at (1,-2) (pulse 2, real, works).
  On arrival the gate fires 2 gt BEFORE A' lands at (1,-1) (the moving block appears at the push
  start, B' only gets power when it lands), so pulse 1 needs either an earlier 2 gt "boost" into B'
  (t3b/t3p: works, window bb = s1-4..s1) or X (2,-1) powered from (3,-1) (t3x: works).
- A''s first pulse can then only come from (2,-2) (the gate owns (0,-2)): the bottom right must
  produce it early. Its only early trigger is an observer (4,-1) watching the BR puller (fed from
  the top at t7): two pulses 4 gt apart; merged by stone (4,-2) + repeater (3,-2) d2 + observer
  (2,-2) east = A' pulse 0 at t15 (t3m: real, works with P_M, shuttle, boost ideal).
- SOLVED in t3n: a redstone lamp at (4,-2) is both the merger's input and a delay: the BR watcher
  lights it at t9 (and t13), it goes dark at t19; observers (4,-3) up, (3,-3) east, (2,-3) east fire
  on both lamp edges, the on-edge copy lands while A' is busy (blocked, harmless), the off-edge copy
  is the boost at t25. t3n: door ideal-rs WORKS (4 positions) with only P_M and the shuttle ideal;
  windows: P_M pm 17..22, shuttle push s1 24..34, pull s2 >= s1+5.

## The last two inputs (left bottom), what fails
- A column of observers down x=-3 from the TL-puller watcher (-1,4) reaches (-3,-1) at t17; P_M is
  then ONE observer (-2,-1) facing west (t19, pockets pk6). The shuttle needs push A1 >= 25 and pull
  A2 >= A1+5: A' (sitting at (1,-1) since the boost) extends at the gate's fire A1+2 (or landing+1)
  and is only placed back at A1+6, and B' must not retract (R leaving (0,-3)) before the next phase.
- Every natural double is 4 gt apart (an observer watching a 2 gt pull-type piston: extend + base
  placement; the shuttle's own gate feedback (-1,-2)->(-2,-2)->SP gives A1+4 AND a loop: the gate
  fires again when R leaves, SP pushes R back in = a clock, t3g "still moving 300 ticks").
- 6 gt doubles (a lamp's two edges) exist, but in the 6 cells left of the shuttle P_M's driver
  ((-2,-1) watching (-3,-1), or (-1,-2) lit from (-2,-2)) and the shuttle's driver ((-3,-3) lit from
  (-3,-2) watching (-3,-1), or (-2,-2) watching (-2,-1)) always share one source, so either both get
  the double or SP's first pulse comes with P_M's (before the boost).
- Pockets on the 6-cell pocket (k up to 6, 78M fillings, pk7) and on the 13-cell pocket
  (coordinator, k up to 4) find each input drivable alone (P_M: 1 block; the shuttle's pull
  re-triggered by R leaving: copper bulb (-1,-2) lit by the redstone block, observer (-2,-2) east,
  dropper (-3,-2)) but no joint filling; the "covers all" sets of pk7 are screen artifacts (they fail
  without assists: the shuttle sticks extended, BUD'd by the dropper's QC).

## Files
wip/t1-t3*.txt + ideal/*.ideal (door ideal-rs WORKS for t1, t2, t3, t3b, t3p, t3q, t3r, t3w, t3m, t3x).

## Shrink attempts on t7b (43)
- `door minimize --seamless full+hall` (with the frame fix, ece7e451): 43 -> 43; without the fix it
  removed the 3 static frame stones (42, frame 9/12).
- t7c: merger replaced by note block (4,-2) + observer (3,-2) + note block (2,-2) (42): the observer
  fires on both of W1's pulses (t11, t15), A gets two pulses: BROKEN. The delay-2 repeater merger stays.
- Walls: 3 blocks per side is the minimum with this topology (the emitter at (-2,2) must point down
  to power ML directly and QC BL; its input must come from TL's park watcher (-1,3)).

## 7x8 with this top: what needs row -4
Row -4 holds D's support (0,-4) and the horizontal channel from the right (cut) side to D's input.
In 8 rows, row -3 can hold no repeater/comparator/dust, so B's >= 7 gt hold must come from a
redstone-block shuttle (two pulses >= 5 apart from the 3 cells left of it: pockets say undrivable in
x >= -2) or a sculk sensor (bridge's t3k-sculk does it in 8x8, sculk category).
