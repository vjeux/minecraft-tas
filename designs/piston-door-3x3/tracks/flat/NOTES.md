# Track flat: "Solid, flat (one z layer), no slime or honey"

Every block in the door plane z = 0, no slime/honey (`door verify` prints `, flat`).

| file | blocks | open/close | note |
|---|---|---|---|
| v2.txt (archive/flat-v2-60b.txt) | 60 | 48 / 48 | first flat door: pist19r mechanism + observer chains |

## Mechanism (19 blocks, mech-synth's `pist19r` with the T-mover on the right)
- 9 door stones; left column on 3 retracted sticky pistons (-2,0..2) facing east (pull-type,
  gap at x=-1); right column on 3 at (4,0..2) facing west; M pulled down by (1,-2) facing up.
- Center: A (1,4) sticky down retracted, B (1,5) sticky down retracted, P_T (3,3) sticky west
  retracted (pull-type: T parks at (2,3)). Everything pulse-driven (2 gt pulses), the SAME
  pulse train on both lever edges: A a1, P_T, B window (on/off), A a2 = b1+2, A a3 = b2+2,
  M >= a3 (on close A's a2 push overshoots T,C one cell down and M's late push puts the
  whole center column back), L and R anytime.
- Ideal-input check: `m19.ideal` (fc2-like timing a1=2): pt 5..9, M >= 22, L/R anywhere.

## Control of v2 (41 blocks)
- Lever (4,5) on observer S (4,4) facing up: S fires at t2 and QC-powers R top (4,2) through
  the empty cell (4,3). O_Y (3,4) east watches S, O_X (2,4) east watches O_Y: its back powers A
  at (1,4) = a1 (t6). P_T is fired by QC: observer (3,5) facing up outputs into (3,4) = P_T's
  quasi cell (t10), chain lever -> (4,6)D -> (4,7)D -> (3,7)E -> (3,6)U -> (3,5)U.
- Top chain (2,7)..(-2,7) east-facing observers, (-2,6) up, (-1,6) west = R_B's input (t20).
  R_B (0,6) d4 -> B by QC through (1,6) (window t28..36); O (0,5) on R_B -> X dust (0,4) on
  (0,3): a2 t30, a3 t38 (fc2's dance cluster, flattened).
- Left chain (-3,6)E (-3,5..3)U: (-3,3) fires the cross dust (-3,2) on (-3,1) = the whole left
  stack (stack-feed in the plane: dust points into the top piston, its floor powers the middle
  one and QC-powers the bottom one) at t26.
- (-4,3)E, (-4,2..-2)U, (-3..0,-2)W: (0,-2) fires P_M at t46 (>= a3).
- R middle/bottom: (5,4)W watches S, (5,3)U, (5,2)U -> cross dust (5,1) on (5,0) (t8).

## Lessons
- Layer numbers are offsets from origin.y (keep origin y = 0 so layer = absolute y).
- Dust on a piston pops when the piston extends (the extended base top is not sturdy).
- A lever is a weak source on all 4 in-plane sides: keep it off every piston and every quasi
  cell (the cell above a piston) - e.g. a lever at (-2,4) QC-powers a left stack at (-2,2).
- In the plane, repeaters/comparators cannot be locked (their sides are N/S).
