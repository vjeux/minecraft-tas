# Track sf-holdA: pist19r with A held extended at rest ("Solid, no slime or honey" board)

| file | blocks | note |
|---|---|---|
| **r41.txt = BEST.txt** | **41** | w42 + `door resynth --palette block,dust,dot,rep,rep4,obs,torch,lamp,bulb --max-size 4`: X dust + floor + the TR chain (2 observers) -> a plain block X' (1,4,-1) (O_a and O' strongly power it: A-high directly, A-low by quasi-connectivity through B's head) + a cross dust (2,4,-1) on a floor (2,3,-1) next to it, pointing into the TR puller (2,4,0). The TR puller now gets X's THREE pulses (t2, t14, t22): three toggles end with TR up, and the right pair (watching TR's cell) toggles three times too. Open 28 / close 26, 32 positions |
| hA.txt | 43 | the A-held wiring of w42 (below) |

Goal: beat wire19sf's w42 (42, slime-free) with a pist19r variant whose close ends with A
EXTENDED and powered (fresh-c's symmetric close, as in slime-free's s7q), so that the bottom-middle
piston M may fire early (w42 spends 4 blocks on M's late pulse).

## Ideal-input results (door ideal, 4-8 positions x 2 cycles)

| file | mechanism | result |
|---|---|---|
| h1-mech.txt / h1.ideal | w42's pist19r (P_T (-1,3,0) east, TR pulled up, right pair, left column of 3) + A extended at rest, RB (1,4,1) south of A, Q (2,4,1) sticky facing west (pushes RB to (0,4,1)) | WORKS: open 24 / close 13 |
| h3-mech.txt / h3.ideal | TL pulled UP (puller (0,4,0)) + left pair, right column of 3, P_T (3,3,0) west (T parks at (2,3,0)), RB (1,4,1), Q (0,4,1) facing east (RB to (2,4,1)); the TL puller fires with Q from ONE powered block (0,5,1) (Q directly, TL puller by quasi-connectivity, updated by Q's motion) | WORKS: open 24 / close 13 |
| h2-mech.txt / h2.ideal | RB (2,4,-1) powering X dust AND a level-type TR puller | FAILS: a redstone block next to X makes the dust connect to it (corner shape), so X stops pointing into A |

Windows (ideal ticks, one variable at a time; real observer ticks are +1):
* h1: **M, left column, right pair, TR puller: ANY tick 0..40** (A held: B's close window is
  blocked by the extended A and A2/A3 are swallowed); q 3..6 (pt 7); pt 6..9 (q 5), i.e.
  pt - q in 1..4; a1 0..4; bon 9..11; a2 13..14; a3 >= 21; boff 18..19.
* h3: q 3..6, pt 6..9, a1 0..4, bon 9..11, left pair / right column / M anytime.

## Why the hold is hard to make pay (count analysis)
s7q (44) IS pist19r with A held (RB (0,4,0) west of A, Q (0,4,-1), P_T (1,3,2) north). Versus
w42: + Q + RB (2 mechanism blocks) + Q's feed (2 observers off the lever) - M's late trigger (4)
+ M's early trigger (observer watching BR + block, 2) = +2. To beat 42 the hold must share:
* Q needs ONE pulse per edge at real t4..t7 (P_T at t8). Every cell next to Q is >= 2 observer
  hops from the lever cluster, except Q (0,4,-1) (s7q: lever on an observer watching it, then one
  more observer).
* A's free neighbours: (0,4,0) (needed by a TL puller or RB), (1,4,1). P_T's quasi cell must not
  touch RB's rest cell, and P_T's head is always directly below RB's rest cell, so a BUD-ed P_T on
  RB's level retracts at once (before T lands).
* RB cannot double as a level source for a level-type edge piston: no cell is next to both A and
  an edge puller; next to X it changes X's shape.

## e41 (41): the same trick on wire19sf's mirrored w42e
`e41.patch` on w42e: X dust + floor + the TL chain ((0,4,-2), (0,4,-1)) -> block X' (1,4,-1) +
cross dust (0,4,-1) on floor (0,3,-1) pointing into the TL puller (0,4,0). Open 28 / close 26,
32 positions. **General lesson: X (A's pulse hub) fires three times per lever edge (p0, A2, A3);
any "anytime" pull-type piston that X can reach through one dust (+ floor) costs 2 blocks and no
chain.** A solid block X' in place of the X dust powers A-high directly and A-low by
quasi-connectivity (through B's head), and needs no floor, which frees (1,3,-1).
