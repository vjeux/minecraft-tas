# Track flat: "Solid, flat (one z layer), no slime or honey"

Every block in the door plane z = 0, no slime/honey (`door verify` prints `, flat`).

| file | blocks | open/close | note |
|---|---|---|---|
| v2.txt (archive/flat-v2-60b.txt) | 60 | 48 / 48 | first flat door: pist19r mechanism + observer chains |
| min-v2.txt (archive/flat-v2-57b.txt) | 57 | 42 / 42 | `door minimize`: a dust step replaces 3 chain observers; observer -> block feeds the right pair |
| v3.txt (archive/flat-v2-55b.txt) | 55 | 36 / 34 | R_B's input = dust under a block powered by the top chain; M fired by an observer watching the bottom-left door block (BL flag), left stack fired late |
| v4.txt (archive/flat-v2-54b.txt) | 54 | 36 / 34 | the R_B-input dust stands on an observer that starts the late left chain |
| v5.txt (archive/flat-v5-52b.txt) | 52 | 34 / 32 | `door synth` (k=9, obs/dust/block/rep1-4/torch, box x -6..-1, y -1..6) re-laid the late left chain + stack feed in 8 blocks instead of 10 |
| v6.txt (archive/flat-v6-51b.txt) | 51 | 34 / 32 | `door resynth --min-size 5 --max-size 6` (palette with rep1-4): a delay-2 repeater replaces two chain observers |
| v7.txt (archive/flat-v7-50b.txt) | 50 | 32 / 30 | `door synth` k=9 on the cut crossing (a1 path + P_T chain + top chain, 10 blocks, box x -1..5, y 3..8): 9 blocks |
| v8.txt = BEST.txt (archive/flat-v8-49b.txt) | **49** | 32 / 30 | lever moved to (5,4) on its observer S (5,3), which fires a cross dust (5,2) on (5,1) = all three right pistons; observer (4,4)E watching the lever starts the a1 chain; X's floor obsidian so the early P_T pulse (t2) is a no-op |

## v8 vs v7
- The right column's feed was 4 blocks (3 observers + smooth stone) on top of the lever's
  observer; now the lever-observer itself drives the stack-feed dust: lever (5,4), S (5,3) facing
  up, cross dust (5,2), floor (5,1). The a1 chain needs one observer more ((4,4)E watching the
  lever, then (3,4)E, (2,4)E -> A, still t6): net -1.
- (4,4)E outputs into P_T's quasi cell (3,4), so P_T also fires at t2. Harmless: on open it pulls
  air (T is lifted at t6); on close it pushes T into C's cell and C into X's floor (0,3), which is
  OBSIDIAN (immovable) in v8, so the push is blocked (stone there gets pushed into the left chain).

## Control of v7 (31 blocks), t = ticks after the flip
- Right (8): lever (4,5) on observer S (4,4) [QC-fires the right top piston through (4,3), t2];
  O_Y (3,4)E, O_X (2,4)E -> A (a1, t6); (5,4)W, (5,3)U, (5,2)U -> smooth stone (5,1): right
  middle (direct) + bottom (QC), t8.
- Crossing (7): (2,5) observer facing down watches O_X and lights the dust (2,6) standing on it
  (t8); that dust steps up-west to the dust (1,7), whose floor is an observer (1,6) in B's QC cell
  (R_B still QC-powers B through that cell). (3,6)W watches the dust (2,6), (3,5)U -> P_T's QC
  cell (3,4): P_T at t12. (0,7)E watches the dust (1,7) -> J' (-1,7) (t10).
- Top-left (6): J' stone, dust D6 (-1,6) under it (gadget dust-below), R_B (0,6) d4 -> B's QC
  cell (1,6): window t18..26; O (0,5) on R_B -> X dust (0,4) on floor (0,3): a2 t20, a3 t28.
- Left (8): S1 (-1,5) observer facing up watches D6 (t12) -> (-2,5)E -> (-2,4)U QC-fires the
  top-left piston through (-2,3) (t16); TL lands in (-1,2), watched by (-1,3) facing down (a
  TL flag) -> (-2,3)E -> delay-2 repeater (-3,3) facing east -> (-3,2)U -> smooth stone (-3,1):
  left middle + bottom late (t26).
- Bottom (2): BL flag: (0,-1) facing up watches BL, powers stone (0,-2) next to P_M: M after a3.

## Synthesis notes (what found the last 4 blocks)
- Hand layouts stalled at 54; `door synth` on a CUT region with the door judge as the target
  found 52 (left chain, k=9) and 50 (crossing, k=9) in minutes; `door resynth` (groups of 5-6)
  found 51. Recipe: `door patch` to clear a connected region of n control blocks, then
  `door synth <cut> --lo --hi (z = 0..0) --from <cells feeding it> --to <cells it must reach>
  --palette block,dust,obs,rep1,rep2,rep3,rep4,torch --k n-1`. Enumerations: k=9 ~150-850k
  circuits (3-15 min on 4-12 workers).
- Searched and empty on v7: minimize --moves all --plateau; resynth 2..4 (466 groups); synth
  k<=6 on the left 7, k<=7 on S1 + left 7, k<=7 on the top-left corner (crossing end + J' + D6
  + S1 + left head, 8), k<=5 on the right feed + a1 path (6).

## Mechanism (19 blocks, mech-synth's `pist19r` with the T-mover on the right)
- 9 door stones; left column on 3 retracted sticky pistons (-2,0..2) facing east (pull-type,
  gap at x=-1); right column on 3 at (4,0..2) facing west; M pulled down by (1,-2) facing up.
- Center: A (1,4) sticky down retracted, B (1,5) sticky down retracted, P_T (3,3) sticky west
  retracted (pull-type: T parks at (2,3)). Everything pulse-driven (2 gt pulses), the SAME
  pulse train on both lever edges: A a1, P_T, B window (on/off), A a2 = b1+2, A a3 = b2+2,
  M >= a3 (on close A's a2 push overshoots T,C one cell down and M's late push puts the
  whole center column back), L and R anytime.
- Ideal-input checks: `m19.ideal` (fc2-like timing a1=2): pt 5..9, M >= 22, L/R anywhere.

## Control of v4 (35 blocks), t = ticks after the flip
- Lever (4,5) on observer S (4,4) facing up: S fires at t2 and QC-powers R top (4,2) through
  the empty cell (4,3). O_Y (3,4) east watches S, O_X (2,4) east watches O_Y: its back powers A
  at (1,4) = a1 (t6). P_T is fired by QC: observer (3,5) facing up outputs into (3,4) = P_T's
  quasi cell (t10), chain lever -> (4,6)D -> (4,7)D -> (3,7)E -> (3,6)U -> (3,5)U.
- R middle/bottom: (5,4)W watches S, (5,3)U, (5,2)U -> smooth stone (5,1) (middle direct,
  bottom by QC), t8.
- Top chain (2,7)E (1,7)E (0,7)E (t12) -> stone J' (-1,7) -> dust (-1,6) under it (dust reads a
  strongly powered block above it) -> R_B (0,6) d4 -> B by QC through (1,6) (window t20..28);
  O (0,5) on R_B -> X dust (0,4) on (0,3): a2 t22, a3 t30 (fc2's dance cluster, flattened).
- The dust (-1,6) stands on observer S1 (-1,5) facing up (fires t14), then (-2,5)E (-3,5)E
  (-4,5)E (-5,5)E (-5,4)U (-4,4)W (-3,4)W (-3,3)U -> cross dust (-3,2) on (-3,1) fires the
  whole left stack at t30 (stack-feed in the plane: the dust points into the top piston, its
  floor powers the middle one and QC-powers the bottom one).
- BL flag: observer (0,-1) facing up watches BL (0,0) and powers stone (0,-2) next to P_M:
  M moves 2 gt after BL on close (t32 >= a3) and 4 gt after it on open.

## The coordinator's round-2 ideas, examined
- **Lever on the left** (a1 via X, left column early from the lever, R_B fed late, right column
  late from it, M watching BR): P_T sits opposite X (both want (0,3)/(2,3)), so with the lever
  on the left P_T needs a pulse at a1+3..7 on the far side: a 10-observer chain over the top
  arrives at t22 (too late) and a dust line needs a floor per cell. The left-lever cells are
  also boxed in: (-2,4) QC-powers the left top piston through (-2,3), (-1,5)/(-1,6) power P_T's
  or R_B's input, (0,6)/(1,7)/(2,6) power B's quasi cell. Paper estimate ~51; not built.
  Moving the lever lower on the right instead gave v8 (49).
- **Three pulses per side column**: valid with ideal inputs (m19-3p/3q/3r), but every source of
  the two late pulses is at the top (O on R_B -> X; an observer in B's quasi cell (1,6) watching
  R_B also works: R_B still quasi-powers B through it), and the bottom piston of either column
  is 6+ cells away: the delivery costs more than the late chain it would replace (the left late
  part is 5 blocks after the top-left fires). The pulse also comes ~10 gt after A's pulse at the
  bottom, outside the dm 1..3 window M's no-op trick needs.
- Observer X (no floor, so T could park at (0,3)): an observer at (0,4) facing west does power
  AND update A in its lower position (its output updates the neighbors of B's head (1,4), which
  include A), but it can watch only one cell, and nothing next to (-1,4) changes exactly at the
  B edges (R_B cannot sit there); a second repeater costs what X's floor saves.

## Findings
- **Three-toggle pistons**: a pulse-driven side stack may get 3 pulses per edge (out, back,
  out on open; back, out, back on close). Ideal inputs (`m19-3p.ideal`, `m19-3q.ideal`,
  `m19-3r.ideal`): the left stack toggled at A's own pulse ticks (a1,a2,a3), M toggled at
  +1..3 (close; open any) works; the first stack pulse may be anywhere in 0..18 if A does not
  get it, or at 13..15 if A gets it too (an extra A pulse between T's park and B is harmless).
  So a left stack fed straight from A's pulse dust X + the BL flag would need no late chain;
  it was not wired because the bottom piston of a 3-stack cannot be fed from X's dust without
  ~10 blocks of dust/floors (net 0).
- An extra P_T pulse at t2 is harmless if X's floor (0,3) is obsidian (on close the push line
  T, C, floor is blocked).
- **M must be late in every flat slime-free pulse-only scheme found**: `door mech-synth
  bases/sidesM.txt --add 4 --lo=-2,3,0 --hi=4,6,0 --kinds sticky --ext --pulses-only --edges
  start --split-only` (66k combinations, 587 mechanisms, 103 symmetric): every symmetric
  sequence with M's pulse early uses a rider (a piston riding a slime column), which a
  slime-free door does not have. So M needs a pulse after A's last pull (or the 3-toggle trick).
- The late left chain is geometrically minimal: 9 observers from S1 (t14) to the stack feed
  (8 would put the stack at t28 = a3 - 2 but no 8-cell path fits around the stack).
- Searches on these designs that found nothing: minimize --moves all --plateau (v2 -> 57, then
  nothing on v3), regrow size 2 (11.8k) and 3 (3.9M candidates, 476k screened), resynth.

## Lessons
- Layer numbers are offsets from origin.y (keep origin y = 0 so layer = absolute y).
- Dust on a piston pops when the piston extends (the extended base top is not sturdy).
- A lever is a weak source on all 4 in-plane sides: keep it off every piston and every quasi
  cell (the cell above a piston) - e.g. a lever at (-2,4) QC-powers a left stack at (-2,2).
- In the plane, repeaters/comparators cannot be locked (their sides are N/S).
- The T-mover always sits opposite X's floor (both want (0,3)/(2,3)), so a1/P_T and the dance
  cluster end up on opposite sides; the R_B input then crosses over the top.
- `pkill -f <pattern>` kills your own shell if the pattern appears in the command line.

## Tools (this track)
- search.rs `same_league`: a flat design stays flat (candidates with a block off the door
  plane are rejected); synth.rs resynth clamps its box to the door plane for flat designs
  (merged to main).
- `door resynth --only x0,y0,z0:x1,y1,z1` (commit d8b42b7): only groups inside a box, so groups
  of 6-8 can be tried on one region (all groups of 5-6 on a 50-block design take ~2 h).
- Gadget `dust-below` (observer -> stone -> the dust under it lights).
- Searched and empty on v8: regrow --size 2 --any (162k), synth k<=9 on the cut a1+crossing+P_T
  region (10 blocks, 193k circuits), synth k<=6 on the left late part + BL flag (7),
  resynth --only on the right region (groups of 5-7, 27 groups).
