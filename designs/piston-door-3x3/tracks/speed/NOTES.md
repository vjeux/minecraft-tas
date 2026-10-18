# Track `speed`: the fastest 3x3 door (open, close, open+close)

Ticks are as `door verify` measures them: open = first tick after which all 9 door cells and the
walkway stay walkable (a `moving_piston` counts as blocked); close = first tick after which the 9
door cells hold their rest blocks (placed, not moving).

## Theory (from the 26.3 source)
* A piston event at tick t: a PULLED block's old cell is air at t; the moved block is a
  `moving_piston` until the block-entity phase of t+2. It can be moved again at t+3 at the
  earliest. A PUSHED block can be finalized early by a spit (retract while it is still moving:
  type 2), placed in the block-event phase of the retract tick.
* Power loss cascades inside one block-event phase (a moved redstone block switches its old
  neighbors off at once); power gain from a moved block only happens when it lands (event next
  tick). A lever flipped between ticks fires adjacent pistons in tick 1.
* Reach-through: a sticky piston that extends into an empty cell and retracts before (or after)
  the head lands pulls the block 2 cells away (the moving head is finalTick'd to air first).
* Topology: corners and edge blocks (T, L, R, M) each have one outward exit cell E. The center C
  has none: it needs 2 moves in a straight line through an edge cell N1 into that edge's exit
  cell E. Its puller must sit in E (reach-through N1), so the edge block must vacate N1 (into E,
  its only exit) and then E, before a piston can enter E. A fixed piston above E (F) can then
  reach through E for C's second move.
* Lower bounds for that topology: T into E at t1 (lands t3); at t4 a pusher shoves [A', T]
  sideways so A' (a sticky piston facing down) takes E, spit-finalized at t4; A' 0-ticks at t4
  (C into N1, lands t6); at t7 A' is pulled back out of E and F 0-ticks through E: OPEN at 7.
  Close: F pushes C from E into N1 with a 0-tick spit at t1, T is pushed back into E at t1, F
  re-extends at t4 (its base landed at t3) pushing [T, C] down together: CLOSE at 6.

## Log
* ideal-1 (scripted redstone blocks, 1-tick pulses instead of 0-ticks): the geometry works:
  T into E (t1), Q pushes [A', T] east (t4), A' pulls C into N1 (t8), Q pulls A' back and F
  pulls C into E (t12): open at 12 with 1-tick pulses.
* ideal-2 (`door ideal ideal-2.sched`): back-push variant (Q at (1,3,2) facing north pushes
  [A', T] so T lands above the front walkway; T-returner TRt at (1,3,-2)): WORKS, open 12 /
  close 7 with 1-tick pulses. Close: F pushes C E->N1 and spits it (t1-2), TRt pushes T into E
  and spits it (t3-4), F (base landed t4) re-extends at t5 pushing [T, C] down: close 7.
  Gotcha: a piston only queues an extend if its push resolves at that moment (a moving piston in
  the way = no event, no retry): TRt must be powered after F's head left E.
* Wiring plan: A' at E fed by an observer watching T's landing cell (1,3,-1) (fires t6-8 off
  Q's push at t4) -> solid block (1,5,-1) -> dust dot on F (1,5,0): powers F and QC-powers A'
  at E only (A' at rest has QC cell (1,4,1): keep every neighbor of it unpowered).
* ideal-4/5 findings: an observer watching T's landing cell drives A' (A' extends the tick it
  lands or is spit). Q driven by a 2gt observer pulse spits A' into E at t6 (A' extends at t6).
  Problems met: (1) A' at E is QC-powered through F's cell, so every F signal also powers A'
  (A' cannot retract while F is powered); (2) Q's silent hold is broken early by A's
  retraction at t8 (forwarded by Q's head), not only by its landing at t10.
* Plan B (sideways geometry, Q from the west, T pushed east): E's free neighbors (1,3,+-1) let
  an observer at (1,3,1) facing south power A' at E DIRECTLY (not through F's cell). Drive it
  with an observer chain from the lever (each observer side-watches the previous one: 2gt per
  link, clean single pulses on both lever edges, position independent).

## Results (all `door verify --extra-offsets 28`, category solid, slime-free)
| file | blocks | open / close | what |
|---|---|---|---|
| w1-123b.txt | 123 | 12 / 19 | first wired ideal-6 (child session, see wire/NOTES.md) |
| w1-114b.txt | 114 | 12 / 19 | `door minimize --max-open 12 --max-close 19` |
| w2-139b.txt | 139 | 12 / 14 | close spit at k6 (observer into F's side), TR at k10 |
| w2-126b.txt | 126 | 12 / 14 | minimized at equal speed |
| w5-140b.txt | 140 | 12 / 11 | F's redstone block returned by a 2nd piston at k6, TR at k7 |
| **w5-122b.txt = BEST** | **122** | **12 / 11** | minimized at equal speed |
Open 12 = the theory bound with 2gt pulses (C into N1 at k8, A' lands k10, Q pulls it k11, F
reaches through k11-12). Below needs 0-ticks.
| w6-161b.txt | 161 | 12 / 7 | F as a BUD: silently quasi-powered in the open state (lock-delayed repeater), pushes C at close k1 when the shuttle piston's head leaves, spits at k2; TR pushes T at k2; F pushes [T, C] home at k5 (close = the topology bound) |
| w6-134b.txt | 134 | 12 / 7 | minimized at equal speed |
| s1..s3 | 128, 125, 116 | 12 / 7 | hand size passes (wire/NOTES.md) |
| **s4-109b.txt = BEST** | **109** | **12 / 7** | Q's silent hold from a single repeater |
| w5-121b.txt | 121 | 12 / 11 | w5 deep-minimized |

## Mechanism of the 12/7 door (s4), in short
* Open: a lever-powered piston shoves the redstone block powering F (and the side/M torches go
  dark through dust): F pulls T up into E at t1, sides and M leave at t1-2. Observer chain off
  the lever (t2, t4) pushes Q at t4: Q shoves [A', T] east, A' takes E. Observer O_A (t6) powers
  A' at E directly: A' extends t7, retracts t8 pulling C into N1. Q is held silently (repeater
  into a block next to Q's quasi cell) and released by A' landing at t10 (through Q's head):
  Q pulls A' back at t11; F, quasi-powered through (1,5,0) (kept air, so A' at E never sees it)
  by a 4-observer chain, extends into the freed E at t11 and retracts at t12 pulling C out: open 12.
* Close: F is silently armed in the open state; the shuttle piston's head leaving at t1 updates
  it: F pushes C from E into N1, an observer pulse updates it unpowered at t2 so it spits C; the
  T-returner pushes T into E at t2 (its update comes from a side piston's head); F's redstone
  block is pushed back by a second piston (lands t4) and F pushes [T, C] home at t5: close 7.
| o2-137b.txt | 137 | 11 / 7 | F 0-ticks at t11: redstone block R_X armed (t6, lands t8) in F's quasi spot (1,5,1); P_X (normal piston in F's quasi cell (1,5,0), silently powered by a second redstone block R_Z landing at t10) is updated by F's extension and shoves R_X away in the same block-event phase, so F retracts at once and pulls C |
| o2-135b.txt | 135 | 11 / 7 | minimized |
| o5-122b.txt | 122 | 11 / 7 | tick-6 pulse from 3 stacked observers |
| **o6-120b.txt = BEST** | **120** | **11 / 7** | pulse chain moved next to its targets |

## Next ideas
* Open 10: move the whole post-push sequence a tick earlier with one more same-tick
  extend+retract (e.g. Q pushes and spits A' at t4 so A' is placed at once and can pull C at t6;
  A' then lands at t8 and the rest follows 2 ticks earlier). Theory bound with 0-ticks: open 7.
* Size: the east dust line to the side torch (~13 blocks), the 4-observer chain for F's
  reach-through, the lock chain delaying F's silent power (6 blocks), individual side pistons.
* Tools added on this branch: `door ideal` prints worst open/close ticks; `door drive planex`
  (side view of an x plane). (`door minimize --max-open/--max-close` is main's.)
