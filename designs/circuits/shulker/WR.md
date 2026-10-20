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

## Speed per 1-wide slice (ceilings)

- **Unloader:** the only way out of X is down. A hopper minecart pulls 20/s, but in-slice it
  straddles at most 2 hoppers (z-edge), which drain 2.5/s each: **5 items/s per slice**.
- **Loader:** one feed face. A gated dropper gives 5/s, but it needs box presence, which has no
  face left to read it: **2.5/s per slice** with a hopper feed.
