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

**Result: a 1-wide loader at hopper pace with a box read is impossible.** What remains:
- (a) Read the feed's backlog instead. The feed must drain faster than it is supplied (otherwise
  the backlog after each swap is permanent, NOTES.md), which caps the rate at about 1.25/s with
  one hopper face.
- (b) Pitch 2.

Target: the smallest pitch-2 loader at 2.5/s with every badge, plus a 1-wide backlog loader as the
1-wide record. Both are still to build.

### Unloader: fits

- Dispenser above X, extraction hopper H below X.
- Push line along z: sticky piston, stone, X, drop cell, collector K.
- Read: a comparator on H. While the box has items, H holds exactly 1 item, because it pushes and
  then pulls in the same tick. So "H is empty" means "the box is empty".
- Timing bound: H goes idle 8 gt after its last push, and a drop still in X's column at that
  moment is lost into the items chest. So the piston must fire within 5 gt of H emptying:
  comparator (2 gt) plus one torch (2 gt).
- That torch must give a pulse only on the falling edge, with no side inputs. The design is a NOR:
  - The torch sits on block B.
  - B is powered by the comparator on H, or by a second torch fed by a delayed copy from a second
    comparator on H (H's other horizontal face, which is free when K sits level with X).

Target: a 1-wide, pitch-1 unloader at 2.5/s with every badge except tileable(>1). Blocks: about 20
to 25, against 41 for the smallest unloader today, which is 5 wide (tileable(5)). In progress.

## Speed per 1-wide slice (ceilings)

- **Unloader:** the only way out of X is down. A hopper minecart pulls 20/s, but in-slice it
  straddles at most 2 hoppers (z-edge), which drain 2.5/s each: **5 items/s per slice**.
- **Loader:** one feed face. A gated dropper gives 5/s, but it needs box presence, which has no
  face left to read it: **2.5/s per slice** with a hopper feed.
