# Track boxtall: narrow flat doors (5 or 6 wide, 1 thick), FULL seamless + hallway, smallest volume

Brief (vjeux, 2026-10-18 20:24Z): size = the community VOLUME (`door properties` volume, lever
excluded); goal: flat (`--shape thick<=1`) doors 5 or 6 wide (at most 1-2 columns beside the
opening on each side over the whole cycle), as short as possible (6x9 = 54, 5x10 = 50, 6x8 = 48).
Coordinator (20:27Z): every door must be FULL seamless + hallway-compatible (SEMI+hall interim).

## Geometry of a FULL seamless flat door (why narrow is hard)

Door plane z = 0, opening x 0..2, y 0..2. The hallway ring S at z = 0 is the 12 cells
face-adjacent to the opening: side cells (-1,y), (3,y), floor (x,-1), ceiling (x,3), y/x 0..2.
* Open state: each of the 12 ring cells holds a door block, a quartz-like block (stone) or air; an
  air ring cell that something passed through is a hole and the cell right behind it (one further
  out) must not hold circuitry. Closed: the door plane's ring is hidden (any circuitry is fine).
* Hallway (props-dyn, ring-aware): every S cell at depth +-1..3 is filled with smooth stone (all of
  them, for a flat door). A slime/honey block that moves while it sits in a door-plane ring cell
  drags those walls (z +-1): no slime/honey in the 12 ring cells when it moves.
* So all STATIC circuitry (wiring, pistons) lives outside the ring and the opening: in the 4
  corners (-1,3), (3,3), (-1,-1), (3,-1) and rows y >= 4 / y <= -2, plus columns beyond the ring
  (x <= -2, x >= 4: only in a 6/7-wide box).

### 5 wide (x -1..3)
* Every side ring cell is in S, so no static piston can stand in rows 0..2 at all: no horizontal
  piston can push or pull a door row (a pull needs a base at x -2 / 4, a push a base in the ring).
  Every door block leaves vertically: each lane's middle block needs a 2-cell move (an elevator,
  fc2's A on B) and the ceiling/floor rings hold only 3 + 3 blocks (+4 corners after row shifts):
  three elevators.
* Top and bottom halves cannot share a wire: the only cells between them are ring cells. A signal
  crosses only by motion (an observer watching a block that moves, e.g. a stone column in a side
  ring pushed by a corner piston: stones in S are hallway wall, not circuitry).

### 6 wide (x -2..3)
* The wide side's column leaves sideways on 3 pull-type sticky pistons at x -2 (heads into the
  ring, gaps at x -1): FULL-clean (the door blocks become the wall, the pistons hide behind).
* The narrow side's middle block (2,1) has no 1-cell exit: (3,1) needs a pull from x 4 or a push
  from (1,1) (inside the opening); up/down needs a 2-cell move (its own elevator), because the
  ceiling/floor ring cells above/below it hold TR/BR.
* The wide column x -2 is full of pullers in rows 0..2: top/bottom wiring cannot pass it either
  (an observer can watch one puller across, if only one puller stands there).

## Tools added
* `door mech-synth --seamless-open`: the macro model's open test also requires the door plane's
  ring to be FULL-clean (door blocks, stone or air; no heads; no circuitry behind an empty ring
  cell; no slime/honey in the ring). Use with `--lo/--hi` box limits.

## Log
* 20:41Z `bases/six-a.txt` (6 wide: left 3-stack, fc2 elevator A/B + P_T at the corner (-1,3), T/TR
  pullers, M/BR pullers): does not open (MR has no exit), as expected; `--add 2` running.

## Results (mechanisms, ideal inputs; not wired)
* `bases/six-c.txt`, `six-e.txt`, `six-f.txt`: 6 wide (x -2..3), 1 thick, open state FULL-clean
  in the door plane (every door block parks in a ring cell or the corner (-1,-1); no circuitry in
  the ring). `mech-synth --oc` paths and `ideal/six-*/oc.ideal` WORK in the simulator (8 positions
  x 2 cycles). six-f: every piston pulse-driven (21 mechanism blocks: 9 door + 12 pistons);
  `ideal/six-f/compressed.ideal` (top and bottom in parallel) opens/closes in 34/34 ticks.
  Mechanism box 6x9 = 54 before wiring.
* Shape: left column on W2/W1 (x -2) + BL pulled down and parked in the corner by a floor shift;
  top: fc2 elevator lifts C (P_T at (-1,3) parks T at (0,3)), TR up; bottom: M, BR down, P_BR
  (3,-1) shifts the floor row west, cd19 elevator drops MR; P_E (-2,-1) shifts the floor back.

## Why it is not wired (the blockers)
1. Two elevators are forced in 6 wide: the middle row can lose only ONE block sideways (pull
   left, or push-through right, never both), so C and one of ML/MR need 2-cell moves.
2. Top/bottom crossing: with FULL seamless no static wire may pass the ring, and x -2 rows 0..2
   hold the side pullers; W2/W1 can only be fired from above, W0 only from below. Every free
   crossing cell found collides with a parking/return piston: an observer watching W1 from
   (-2,0) outputs into P_E's cell; dropping W2 or W0 for a QC power block breaks ring capacity
   (9 blocks for exactly 9 usable ring cells) or the close-time row returns.
3. No symmetric pulse train within 3M state pairs (`--sym-bfs`); edge-specific control for 12
   pistons would be ~40 control blocks -> ~6x14 at best.
* `mech-wire --box -2,-8,0:3,10,0 --forbid-ring` on compressed.ideal: no lever spot in the flat
  box (and the crossing above makes a full route impossible as laid out).
* 5 wide is worse: three elevators and no possible crossing except moving stones in a side ring.

## Next ideas
* A crossing that IS the mechanism: the top pushes lane 0 down (BL into the floor ring, watched
  from below) with the floor/corner capacity re-planned; or a lamp-supported lever in an empty
  wide-ring cell watched by observers at x -2 (lever is excluded from circuitry and volume).
* Make one region pulse-symmetric (abpt21r-style overshoot on the top: [A1][P_T][B1][A1][B1][A1],
  derived by hand, needs P_M's push last) and search the bottom alone with `--sym-bfs`.
* 7 wide FULL is much easier: flat-b-v7's mechanism with the right pair top-fired from (4,2) and
  an observer at (4,-1) watching E0 is a clean crossing (volume ~7x10).
