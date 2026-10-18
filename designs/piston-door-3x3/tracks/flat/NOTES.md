# Track flat: "Solid, flat (one z layer), no slime or honey"

Every block in the door plane z = 0, no slime/honey (`door verify` prints `, flat`).

| file | blocks | open/close | note |
|---|---|---|---|
| v2.txt (archive/flat-v2-60b.txt) | 60 | 48 / 48 | first flat door: pist19r mechanism + observer chains |
| min-v2.txt (archive/flat-v2-57b.txt) | 57 | 42 / 42 | `door minimize`: a dust step replaces 3 chain observers; observer -> block feeds the right pair |
| v3.txt (archive/flat-v2-55b.txt) | 55 | 36 / 34 | R_B's input = dust under a block powered by the top chain; M fired by an observer watching the bottom-left door block (BL flag), left stack fired late |
| v4.txt = BEST.txt (archive/flat-v2-54b.txt) | **54** | 36 / 34 | the R_B-input dust stands on an observer that starts the late left chain |

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
