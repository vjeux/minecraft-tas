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
