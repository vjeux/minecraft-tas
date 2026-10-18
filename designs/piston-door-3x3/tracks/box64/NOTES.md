# Track box64: a 1-wide (flat) FULL-seamless door in an 8x8 (or 9x7) box

Goal (vjeux 20:20Z / coordinator): flat (thick<=1), volume <= 64, FULL seamless + hallway
(`--seamless full+hall`), slime-free preferred. Starting points: flat43-full (9x9 = 81, FULL+hall),
flat-b-v7-full (8x10 = 80), flat-seam's e89 (8x9 = 72, cd19 family). Box split (coordinator 21:25Z):
this track owns 8x8 = 64 and 9x7 = 63.

**Result: no 8x8 or 9x7 door yet.** What this track established instead (all with ideal inputs,
`door ideal`, 32 positions unless noted), and why the wiring, not the mechanism, is the wall.

## 1. Ideal-input facts (files in ideal/)
- `m19-88.ideal` (flat track's m19 = the flat43/pist19r mechanism, every input inside x=-2..5,
  y=-2..5): WORKS at 32 positions. B (top elevator (1,5)) fed from a redstone block at (2,5),
  i.e. from B's side: A (1,4) is quasi-powered through (1,5) but is never updated by a block
  placed there, so it stays put and B pushes it. Windows (a1=2): pt 6..9, b1 10..12 (a2=b1+2..3),
  b2 19..20 (a3=b2+2..4), M >= 22, L/R anywhere. So the claim "pist19r needs B fed through row 6"
  only holds for sources that UPDATE A (an observer or repeater next to B updates B's neighbours);
  a strongly powered conductor next to B (fed by something not adjacent to A) is fine.
- `m2.ideal` (M2: pist19r with T parked LEFT at (0,3) by a parker (-1,3) facing east, right
  PAIR + TR pulled up by (2,4); box x=-3..4, y=-2..5): WORKS at 32 positions, same windows.
- `m2p.ideal`: the top elevator also works with TWO 2 gt pulses on B instead of a window (pulse 1
  pushes A down and spits, pulse 2 extends into the gap and pulls A back): b1=12, b2=17..20 with
  a2=b1+2, a3=b2+4; and with a2 = b1+4, a3 = b2+4 for b2-b1 = 7..8 (one pulse train {b1,b2}
  driving B directly and A through a 4 gt delay).
- `m5.ideal` (M5 = cd19 center-down + left 3-stack + right 3-stack + T puller (1,4), P_M (3,-1);
  box x=-2..5, y=-3..4, all inputs in the box): WORKS at 32 positions. Windows (a1=0): pm 3..10,
  b1 8..12, a2=b1+2..3, b2 19..20, a3 22..24, top >= 22, L/R anywhere.
- `m5p.ideal` (two-pulse B'): a2 = b1+4, a3 = b2+4 WORKS for b2-b1 = 7..10; a2=b1+5, a3=b2+5 for
  8..10; window mode with a2=b1+4, a3=b2+4 works for windows of 9..12 gt. So the side that only
  WATCHES B' (observer next to B' -> observer -> X block next to A') can deliver a2/a3 in time:
  that is a working crossing of the blocked column x=1 for the X role.
- Extra pulses on A are fatal everywhere except while A is down and out of reach (m2, A at 17).

## 2. Why 8x8 is a wiring wall: the plane splits into regions
In one plane, the door, its swept ring cells and the pistons cut the box into regions that can only
talk through the mechanism (an observer watching a piston, a door block's rest cell, or a moving
block) or through quasi-connectivity. Ring cells are walls: open, they must hold door blocks,
quartz-like blocks or nothing (FULL), and a quartz-like block only powers its own neighbours.
- pist19r (flat43) in x=-2..5: RIGHT (the x=5 column joins top and bottom), TOP-LEFT (feeds L2/L1
  through (-2,3), sees only A and B), BOTTOM-LEFT (feeds L0 through (-2,-1), sees only P_M and L0).
  In x=-3..4 the right side splits instead. In 7x9 both bottom corners are unreachable.
- cd19 (center down) in 8 rows (no row -4): A' (1,-2) has two side cells, one must deliver a1
  (early) and the other must host O (the observer that watches B's driver and powers X next to
  A' (1,-1)); O's side also drives B'. Whichever side holds the lever, the other side needs a
  crossing at about a1+4, and the only thing moving then is A', whose watcher cell is O's cell.
  The B'-watcher crossing (section 1) fixes a2/a3 but comes after b1, too late for P_M (pm < b1)
  and for B' itself.
- Side-column parity: a door column needs an odd number of short pulses per flip; watching a
  piston or a pull-type gap double-fires (the retraction finishes in the block-entity phase after
  the watcher's off tick); watching an elevator gives exactly two clean pulses (b1+2, b2+2).
  flat-seam's pulse merger (observer on a delay-2 repeater) fixes doubles up to 4 gt apart; the
  B-watcher's two pulses are 7..10 apart, which needs a delay-4 merge + waxed copper bulb + observer
  (3 blocks + the repeater's floor), and the regions that need it have no room for that.
- Bottom row: in an 8-row box the bottom row cannot hold dust or repeaters (their floor would be
  outside), which kills most mergers next to B'.

## 3. What was run
- `resynth --fit=-2,-2,0:5,5,0 --seamless full+hall --extra 3 --margin 2 --max-size 4` on
  flat43-full: nothing (148 groups; the row-6 crossing cannot be re-grown locally). logs/fit88a.log.
- Ideal sweeps above (ideal/*.ideal), plus extra-pulse tolerance on A (all fatal but one).
- Hand layouts for M2 (x=-3..4) and M5 (x=-2..5): every attempt ran out of cells or hit a parity
  problem in the small regions (top-left: a1, a2, a3 all through (0,4) need a 3-pulse driver;
  bottom-left: lever watcher, a1, pm, B' two pulses, L0 in 7 cells).

## 4. Next ideas
- 8x8 with the extra column on the RIGHT (x=-2..5) and the bottom cluster on the right: RIGHT
  watches A' only through (2,-2), so move O off A's side: X (2,-1) powered by an observer at (3,-1)
  facing east watching a COPY of R_B at (4,-1) (facing east, input (5,-1), floor (4,-2)) fed in
  parallel with R_B; then P_M must be left (M parks at (0,-1)) and the bottom-left needs pm from a
  crossing (A'-watcher (0,-2) + merger), which needs a repeater floor in the 2-row bottom-left.
- A lamp powered 4 gt makes an observer fire twice 8 gt apart: a cheap source of B's two pulses
  (b1, b1+8) from one input pulse.
- A region/crossing analyzer over families' CATALOG (which pistons each region can drive, which
  pistons are visible from two regions) to rank the 97 8x8 mechanisms by wireability.
