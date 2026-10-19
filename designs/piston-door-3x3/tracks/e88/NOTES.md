# Track e88: an 8x8 = 64 flat FULL-seamless door from e89b (2026-10-19)

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
