# Track gravity-2: a cheaper gravity door (sand door blocks)

Board: gravity (the 9 door blocks are sand). Start: synth-g42 (42 blocks, open 5 / close 17).

## Results (all `door verify --extra-offsets 28`, category gravity)
| file | blocks | open / close | what |
|---|---|---|---|
| `g33.txt` = `BEST.txt` | **33** | 5 / 16 | g35 with S1 as the right door column's rest support above an x=2 chain, right backbone ends at y=0 over a fixed obsidian; S1 trigger = 2 observers + 1 block (door synth) |
| `g35.txt` | 35 | 5 / 16 | g36 with the lever hung on the back of P_L (no ceiling block) |
| `g36.txt` | 36 | 5 / 16 | first wired version of the new layout (trigger found by `door synth`, 4 blocks) |

## g33 layout (z = 0 is the door plane; x right, y up)
```
 y\x  -1  0  1  2  3        z=-1 row at y=-1:  x=0 o (observer facing south, watches the RB cell)
   3   s  P  L  P  s                           x=1 A (observer facing west, watches o)
   2   s  S  S  S  s                           x=2 C (block: A powers it, it powers S1)
   1   s  S  S  S  s
   0   s  S  S  S  s
  -1   s  R  h  D  B        R redstone block (rides the left structure), h R3's head,
  -2         P  s           D = S1 (sticky, facing down, the right column's rest support),
  -3            s           B obsidian (open support of the right column, slime never drags it)
  -4         F  s  O        P (y=-2) = R3 (regular, extended), F = S2 (sticky, facing up),
                            s at x=2 = the chain, O = observer riding the chain (fires S2)
```
* **Sides**: P_L (0,3,0) facing west and P_R (2,3,0) facing east sit ON TOP of the side columns
  and push slime backbones (left x=-1, y=-1..3; right x=3, y=0..3). The lever hangs on the back
  of P_L at (1,3,0) (face=wall, facing=east): it touches both pistons. Side control: 1 block.
* **Left supports + R3 control**: the left backbone drags a redstone block from (0,-1,0) (rest
  support of the left column) to (-1,-1,0) (its open support). At rest it quasi-powers R3 through
  R3's head cell (1,-1,0); the side push carries it away at t1, so R3 retracts at once, and on
  close R3 is powered again only once the tower has lifted it to its top cell (self-waiting).
* **Right supports**: rest support = S1 itself (a piston does not move when the structure does);
  open support = obsidian at (3,-1,0) (immovable: the backbone's slime cannot drag it).
* **Middle column (drop tower)**: R3 (1,-2,0) regular, extended, head under M; S2 (1,-4,0) sticky
  facing up with a gap above; a 3-slime chain (2,-4..-2,0) touching S2 and R3's top cell; S1
  above the chain pushes it down / pulls it up; observer O (3,-4,0) rides the chain and powers
  S2 through the chain's bottom slime every time it lands.
  * open: R3 retracts (t1, redstone block gone), S1 pushes the chain down (t5, drags R3 and S2),
    O lands and fires: S2 (now in its lower cell) pulls R3 down one more; the sand falls 3 cells
    onto R3 (lands ~t15). Open after 5 ticks (the falling sand is an entity, not a block).
  * close: S1 pulls the chain up (drags S2 and R3 + the sand on R3), O lands and fires: S2 pushes
    R3 + sand up and spits; R3 reaches the top and is quasi-powered by the returned redstone
    block, extends and pushes the sand into the door. Closed after 16.
* **S1 trigger**: observer at (0,-1,-1) watches the redstone block's cell (t1 -> fires t3),
  observer (1,-1,-1) watches it (t5) and powers block (2,-1,-1) next to S1. S1 moves in tick 5
  on both edges: push on open, pull on close (its own state decides).

## Why the tower is symmetric (t2 / t3 studies)
* With a 3-slime chain, S1's move drags R3 (still at the top) and S2 together, so the open order
  is R3, S1, S2 (S2 in its LOWER cell pulls R3) and the close order is S1, S2 (in its UPPER cell,
  pushes R3 and spits), R3. S2 only needs power right after each S1 move: the riding observer.
* S1 timing windows (ideal sweeps, `t3.sched` / `t5.sched`): open: S1 must act in tick 4-5
  (R3 finishes retracting in tick 3's block-entity phase, so a tick-3 push leaves R3 behind) or
  at >= 12 (after the sand has landed on R3; it falls again); 6-11 breaks the falling sand on a
  moving piston. Close: any tick 1..16.
* A single observer off the redstone block fires in tick 3 (too early): hence two observers.

## Baseline searches
* `door resynth` on g42 (sizes 2..5, palette with lamps, note blocks, bulbs): nothing.
* On g35: `door minimize --moves all --plateau` and `door resynth` (2..4): nothing.
* `door synth` on t2 (pulse train for S2-lower + S1 with fixed observers, k <= 7): nothing (the
  riding observer was the missing piece).

## Mechanics learned
* A wall lever can hang on the BACK of a piston: the back face stays full when it extends.
* A piston only re-checks quasi-power when it gets a block update: a redstone block placed next
  to S2's head cell (not next to S2) did nothing.
* Falling sand that lands on a moving piston breaks (entities): the tower must finish moving
  before the sand arrives (~t13) or move only after it has landed.
* Observers watching a block that a moving structure carries away fire 2 ticks after the move
  starts (t3); an observer that rides a chain fires again when it lands.

## Searches on g33 (it is a local minimum for all of them)
* `door minimize --moves all --plateau`, `door resynth` sizes 2..5 (palette with lamps, note
  blocks, comparators): nothing.
* `door regrow --size 3 --any` (cut any 3 control blocks, regrow 2): 11.8M candidates screened in
  30 min, none passes.
* `door synth` for S1's trigger: no circuit of <= 2 blocks exists in its model; 3 is the minimum
  (two observers + a block, two mirror layouts).
* Verified at 128 positions (`--extra-offsets 124`) and rotation-safe (main's new check).

## Ideas that did not pan out (counted or tested)
* S1 fired by ONE observer off the redstone block's cell: it acts in tick 3, while R3 is still a
  moving block, so the chain leaves R3 behind (t4 trace). Every delay element is even (2 gt), so
  the earliest single-hop trigger is too early and two hops are needed.
* The middle support dragged away by a side structure (no R3): the support returns with the
  side structure at t1 on close, before the tower can lift the column (it would push the fallen
  column's top block).
* A redstone block arriving next to S1 when open (S1 as a level piston): no cell next to S1 can
  receive a block carried by either side structure without gluing S1 itself.
* Lever as the middle support with side pistons at the bottom: the column could never fall.
* Two middle and right columns on one platform lifted by one tower: 14 blocks > push limit 12.
* Replacing the riding observer O by a fixed pulse: a fixed cell cannot reach S2 in both cells
  without also powering R3 at (1,-4,0).

## Next ideas
* A side structure that does not need its own open support (the right obsidian) or the left
  y=-1 slime: e.g. a column that rides onto a piston head when open.
* A tower whose chain mover acts on a level signal with a free 4-tick delay (lamp off-delay is
  4 gt, but lamps power nothing; a torch needs a supported attachment).

## Round 2 (coordinator: port the tricks to solid / flat; gravity sides without obsidian / bottom slime)

### Gravity: the right obsidian and the left y=-1 slime stay
* `door synth` on g33 with the obsidian AND S1's 3-block trigger cleared (k <= 3, palette observers,
  blocks, obsidian, glazed terracotta, dust, repeaters, lamps, note blocks, floors block/GT,
  watching every cell that moves): nothing. The obsidian cannot become the trigger's last block:
  no observer can both watch a cell that changes at t1 and point into (3,-1,0).
* Why the supports are pinned: the open support of a side column is the cell its own backbone's
  y=-1 slime would vacate, so (a) a foot block needs that slime (left: the RB foot), (b) a fixed
  block there must be non-sticky (right: obsidian), and (c) any piston next to that cell is glued
  to the backbone at rest (so a riding redstone block cannot fire S1 there either).
* Bottom side pistons (g10's free supports: body at rest, head when open) would save 3, but the
  only cell touching both is (1,-1,0): a lever there holds the middle column up (sand rests on a
  lever) and blocks its fall. Top pistons + lever between them is the cheapest control.
* Tower: 3 vertical pistons are needed (3 cells of lift on close); the chain must touch R3's top
  cell and S2 (3 slimes); a slime between S2 and R3 does not work (a pulled piston does not drag
  the slime next to it: only sticky blocks in the moving set pull their neighbours).

### Solid: the lever between top side pistons conflicts with the elevator
* `solid/sz-mech.txt` + `sz.sched` (ideal): side pistons at (0,3,1)/(2,3,1) pushing 5-slime
  backbones, lever (1,3,1) on P_L's back (sides + lever = 13 blocks, lever-direct), fc2/b38's
  middle (A (1,4,0), B, P_T (1,3,-2), P_M). FAILS: the lever touches (1,3,0), A's low position, so
  A is held extended there during the UP dance. Moving the lever row to z=2 costs 2 slimes
  (sides 15): mechanism + lever 28, and b38's middle control is 10 -> 38 at best.
* **sz2 (validated with ideal inputs, 32 positions):** `solid/sz2-mech.txt`, `sz2-sym2.sched`.
  Same lever-direct sides (backbones (-1,0..2,0)+(-1,2,1)+(-1,3,1) and mirror), T only goes UP
  (pull-type A_T (1,4,0); T stays at (1,3,0), the lever cell next to it is harmless), and C and M
  go DOWN with a bottom elevator: A' (1,-2,0) sticky up rides B' (1,-3,0); M is pulled down by
  A' and aside by P_M2 (1,-1,-2) facing south (parks at (1,-1,-1)). Mechanism + lever = 26.
  ONE pulse train works on both lever edges (fc2-style symmetric control):
  A_T pulse anywhere in 1..12; A' pulses at x1 (1..4), at b+2 (A' high) and at b+8+2 (A' low);
  P_M2 pulse at b-2 exactly; B' window [b, b+8] (w >= 7), e.g. x1 3, P_M2 7, B' 9-17, A' 11, 19.
  Close: x1 pushes C up from (1,-1,0) (spit), B' lifts A', the b+2 pulse pushes C to (1,1,0),
  P_M2 pushes M back, the last pulse pushes M up; A_T pushes T down whenever it fires.
  Wiring estimate: A_T's pulse 3 (note block next to the lever, observer, observer into A_T),
  the bottom chain like fc2's top cluster (source observer under a door corner, hub dust + floor
  reaching A' in both cells, O_b, J, R_B delay 4, O, a delay-3 tap for P_M2 at b-2) 8-9: 37-38.
  Not wired: both bottom-corner watchers output into (0,-2,0)/(2,-2,0), which are neighbours of
  B''s quasi cell (1,-2,0) (B' would fire with x1), and no observer can feed the hub dust
  (1,-1,1)/(0,-1,0) from a cell that moves at t1 without being glued to a backbone.

### Flat: nothing to port
The flat board has no slime, so there are no backbones for top side pistons to push, and the flat
leaders already hang the lever on an existing observer (no support block to save).

### Re-check under main's front/back rule (f3c5884: the 3x3 columns in front of and behind the opening must be air at rest)
g33, g35, g36 still verify at 32 positions (rotation-safe). sz2's mechanism has nothing at x 0..2, y 0..2 off the door plane (P_M2 is at y=-1, the lever row at y=3) and still passes its ideal schedule at 32 positions.
