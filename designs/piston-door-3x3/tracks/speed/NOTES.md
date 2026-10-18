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
