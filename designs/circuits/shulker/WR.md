# Study 8 world-record targets (shulker loaders and unloaders)

vjeux 2026-10-20: "world record kind of implementations ... 1 wide tilable ... quality over quantity".
Each target below comes from the mechanics (VARIATIONS.md, NOTES.md). It is either met by an
archived design or proven impossible here. Designs that miss the bar get folded or hidden.

## What a pitch-1 tiling (`tile: 1 0 0`) forbids

Copy k sits in the slice x = k, so every block's x-neighbours are the same block in the
neighbouring copies.
- **No redstone dust.** It joins the neighbours' dust along x, so the copies would share signals.
- **No diode side inputs.** Comparators and repeaters face along z, so their sides are the
  neighbouring copies. That rules out subtract comparators and repeater locks. Logic is limited to
  plain comparators, repeaters, torches, observers, copper bulbs and pistons.
- **No powered conductor, torch or redstone block in the cell above a piston, dispenser or
  dropper.** That cell is x-adjacent to the neighbour's QC cell, so it would trigger the
  neighbour's block too.
- **No slime or honey.** Each copy's block sticks to its neighbours, so one piston would move
  every copy.
- What stays safe:
  - Judge chests placed side by side remain single chests: `ChestBlock.updateShape` never merges
    two singles.
  - Each hopper pulls only from the cell above it, inside its own slice.
  - Box drops (spawned at the centre ±0.25) must be picked up before they drift across x = ±0.5.
    The drop stress checks this.

## Faces of the box cell X in a 1-wide slice: 4 (up, down, north, south)

### Loader: needs 5 roles

The five roles are:
- placement: a dispenser
- feed: a hopper or dropper facing X
- read: a comparator, horizontal only, so north or south, either directly or through one
  conductor
- break: a piston and its push line
- drop exit: on the far side of the push line

**Proof that no two roles can share a face:**
- A comparator is not solid, so a drop pushed onto it rests on it and is never collected.
- Reading through the dispenser or a hopper reads that block, since both are containers.
- A drop pushed into the feed hopper gets stuck there: a box can never go into a box.
- The read-through stone cannot also be the pushed stone, because its comparator would have to
  stand where the piston is.
- A drop squeezed out sideways can leave through x into the neighbour's slice.

**Result: a 1-wide loader at hopper pace with a box read is impossible.**

**Reading the feed instead (coordinator 2026-10-20) runs into the same wall.** In a pitch-1 slice
every signal is binary: the only analog-sensitive elements are dust (which the copies share) and
comparator side inputs (which face the copies). That rules out three readings:
- "Full" read off the box itself is 15 against 14, a threshold.
- "Full" read off the feed hopper's backlog needs a threshold too. While the box accepts, the feed
  hopper holds exactly 1 item. That reads level 1 for 64- and 16-stacks but level 3 for an
  unstackable, and 2 items read level 1 again for 64-stacks until 23 items.
- An observer only sees a comparator turn on or off.
So no binary signal says "full" for every item kind.

What remains:
- (a) Time it. Hopper pace is exact: a 64-stack box is full 13824 gt after its first item. That
  passes the base scenario but loses the 16-stack and unstackable badges to their time caps.
  Coordinator 2026-10-20: build it, labelled "64-stack items only" (a copper-bulb ripple counter).
- (b) OPEN PATH, parked: a feed that drains faster than it is supplied, so that "feed not empty for
  a while" means "full".
  - The feed is a dropper fired by a clock and gated by its own comparator. The gate must never
    fire into an empty cell, and the feed hopper is locked during swaps so no backlog is left to
    drain.
  - Blocker: the dropper's comparator and the clock need the feed stack's two horizontal faces, so
    the gate has to cross over or under the box column.
  - Going up needs torch towers that would toggle once per item and burn out. Going down needs a
    piston elevator, a sticky piston moving a stone onto a torch, which worked in a draft.
- (c) Pitch 2, with a box read: all badges. Coordinator: build it as the general-purpose record.

The read-through test, and why it needs pitch 2:
- A comparator reading a container through a block outputs that container's level, unless the
  block is powered at 15; then it outputs 15. With an empty container behind the block, that
  gives "15 or 0": a real ≥15 test on whatever powers the block.
- It needs three horizontal sides around that block: the reader, the container, and the
  comparator that powers it at the tested level. A 1-wide slice has two.

### Unloader: RECORD, `shulker-unloader-1wide`

24 blocks, 1 wide, `tile: 1 0 0` (pitch 1), robust, 2.50 items/s (hopper pace), badges always-on,
16-stack, unstackable, colours, box-starved, drop-free, tileable(1). It replaces the 41-block,
5-wide `shulker-small-unloader`, which now folds under it.
- Layout:
  - dispenser above X, emptying hopper H below X
  - push line along z: sticky piston, stone, X, then a drop cell over the output hopper (that
    hopper catches anything in the drop cell's column, so the drop has no time to drift across x)
- Read: one comparator on H. H holds exactly 1 item while the box still has items, because it
  pushes and then pulls in the same tick.
- Fast path: repeater, block B, torch on B, then the piston, for 3 gt from "H empty" to the break.
  H is still on its 8 gt cooldown until the drop has been pushed out of its column.
- End of the pulse:
  - An observer under the comparator toggles a copper bulb on every change of the comparator.
  - The bulb is unlit while H is empty. Its comparator and one torch then keep B powered until the
    next box's first item.
  - Every comparator starts at 0, so the design is consistent as placed. An earlier draft, where
    a comparator was placed already on, settled differently in one rotation: a 1 gt torch flicker
    made the sticky piston spit its stone into the box cell.
- Start and refills: a comparator reads the dispenser through a stone, and observers watching it
  power the boxes hopper above the dispenser. That fires the dispenser by quasi-connectivity: an
  observer's output into a hopper is no power source for the neighbouring copies.

#### Shrink attempts (vjeux 2026-10-20: "still pretty big")

Budget of the 24 blocks:
- 5 for the box path: boxes hopper, dispenser, emptying hopper, output hopper, drop wall.
- 2 for the break: sticky piston, pushed stone.
- 11 for "empty" to the pulse: comparator, repeater, 4 stones, 2 torches, bulb, its comparator,
  observer.
- 6 for the dispenser triggers: 2 after each break, 4 for start and refills.

Results:
- Greedy in-game shrink (`circuit shulker shrink`, every badge, rate at least 2.45/s): no single
  block can go.
- Two-observer clock firing the dispenser every 4 gt, in place of the 6 trigger blocks (20
  blocks): fails. Two observers facing each other never start as placed, so the dispenser never
  fires.

Why the obvious shortcuts fail:
- Piston next to X, with its head breaking the box (saves the pushed stone):
  - Whatever fires the dispenser strongly from above the piston makes the dispenser a powered
    conductor.
  - That quasi-powers the piston, which gets its update when its head leaves X, so it re-extends.
  - Firing through the boxes hopper does not work from there: an observer above the piston
    outputs upward, into a block that is not the hopper, and the dispenser then gets no update.
- One pulse per box from a torch-inverted copper bulb (n, block, torch, bulb, observer: 5 blocks
  instead of 11):
  - An observer pulse is 2 gt, and a sticky piston spits its stone on 2 gt, so a delay-2 repeater
    has to stretch it.
  - In a 1-wide slice that repeater can only stand behind the piston, and the torch needs a free
    face of the comparator's block. No layout found where all of them fit.

## Pitch 2 (`tile: 2 0 0`): what crosses between the two columns

A diode or observer that moves a signal along x always takes its input from, or puts its output
into, the neighbouring station's column. Dust connects across, and a powered block powers both
columns. Dynamic signals therefore cannot cross between the columns, except:
- Items: a hopper in column 1 pushes west into X.
- A comparator in column 1 facing west reads X directly. Its output goes into the neighbour's
  box, harmlessly.
- A wall torch in one column attached to a block in the other. It turns off when that block is
  powered. The neighbour's torch hangs on its own block, so it is unaffected.
- Constant 15 side inputs. A redstone block in column 1 feeds the side of a column-0 comparator,
  and the neighbour's identical one feeds the other side with the same 15.

The box read and the drop exit can share one face of X:
- Compare-mode comparator next to X, with a redstone block on its column-1 side: 15 only when the
  box is full.
- The moving stone pushes the drop out over that comparator.
- A hopper under the comparator collects it. The hopper's pickup box covers the cell above it,
  comparator included.
- The hopper does not pull from the box, which is not above it.

What that leaves:
- The piston breaks from the opposite face.
- Feed and placement take up and down: a dispenser below X facing up, its boxes hopper in
  column 1, and the boxes chest on X's east face. A chest does not mind the redstone block next
  to it.
- The open problem is routing the full signal from the read side to the piston side, around X.

## Loader with a real box read: RECORD at pitch 3, `shulker-loader-pitch3`

25 blocks, copies every 3 blocks (`tile: 3 0 0`), robust, 2.50 items/s. Badges: always-on,
16-stack, unstackable, colours, restart, top-up, box-starved, drop-free, tileable(3). That is
the 31-block 5x5x5 auto loader's set plus tiling. The auto loader stays shown for its lag: 3
hoppers awake against 4.

The middle column is a free crossbar: diodes there move signals between the two outer columns
without touching a neighbour.

Box cell X:
- Feed hopper above.
- Dispenser below, facing up. Its boxes come from the chest on column 1 through two hoppers.
- East: compare-mode comparator with a redstone block on its side, 15 only when full. Its output
  goes into column 2.
- North: the pushed stone of a sticky piston.
- South: the drop cell over the output hopper.

Logic:
- Full line: dust, then two repeaters north along column 2, then a repeater west into the stone
  behind the piston.
- The dispenser fires on any change of its contents: a comparator reads it through a stone, and
  two observers power the stone under the dispenser.
- It also fires after every break: an observer is the support of the first full-line repeater,
  and the observer watching it powers the read-through stone.
- The in-game shrink removed the drop cell's south wall (26 to 25).

Open: fewer hoppers awake would help. The boxes chest cannot sit on the dispenser's hopper,
because the read comparator holds that cell.

### With restart AND flush: RECORD `shulker-loader-pitch3-flush`

40 blocks, tileable(3), robust, every badge but mixed. That is the restart-flush-loader's set
plus tiling. The in-game greedy shrink removes nothing.

Mechanism: the 25-block loader above, plus an hourglass.
- A chest holds 420 stone over a hopper. A comparator on the items chest locks that hopper while
  the items chest has items.
- Once the items chest is empty, the hopper drains the stone chest into a sink chest at 8 gt per
  item. 420 items take 3360 gt: longer than the restart pause (3000 gt), inside the flush
  deadline (6000 gt).
- When the stone chest is empty, its comparator turns off and the wall torch on its output stone
  lights. A dust staircase down column 2 feeds a repeater into the full line's dust, and the
  piston breaks the last box.
- The hourglass comparator starts at the chest's level as placed (nbt OutputSignal 4; redsim
  models it). The torch starts unlit.
- The hourglass runs out once per batch; refill the chest to reset it.

## Speed per 1-wide slice (ceilings)

- **Unloader:** the only way out of X is down. A hopper minecart pulls 20/s, but in-slice it
  straddles at most 2 hoppers (z-edge), which drain 2.5/s each: **5 items/s per slice**.
- **Loader:** one feed face. A gated dropper gives 5/s, but it needs box presence, which has no
  face left to read it: **2.5/s per slice** with a hopper feed.
