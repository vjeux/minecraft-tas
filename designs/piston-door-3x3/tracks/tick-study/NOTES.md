# Track `tick-study`: 0-tick and 1-tick gadgets, floors for the 3x3 door

The study is `docs/ticks.md` (catalog, components table, pulse generators, floors with proofs and
schedules). Gadgets in `designs/gadgets/` (rows in `docs/mechanics.md`), all passing `door drive`
and identical in the official server at 4 positions (`door vanilla-gadget`, logs/ here).

## Results
* Floors (solid): **open >= 4**, **close >= 1** (open + close >= 5). Boards: 10 / 7.
* **`open-4`**: a whole 3x3 opening clear at tick 4 (ideal inputs), the open floor. Critical path
  alone: `center-exit`.
* **Close 3 on the same machine** (`open-4-close3.drive`): C and T both placed at k2, TL at k3.
  Two carried-piston chains started at one tick boundary interleave in the FIFO block-event queue
  one event per chain per round (a failed push never retries), so T's chain is padded with one
  piston (D1 fires Y_P, whose partner sits in its quasi cell) to run after C's. Without the
  padding: close 4 (`open-4-cycle.drive`). Pieces: `center-return`, `corner-return`.
* New tool: `mctas door vanilla-gadget <drive>...` (crates/piston-door/src/vanilla_gadget.rs) and
  the drive command `setblock` (the game's /setblock, identical in both worlds).

## Gadgets (21 new)
zero-tick, zero-tick-normal, zero-tick-reach, zero-tick-onplace, zero-tick-observer,
zero-tick-pulse, double-move, instant-off, instant-on, event-order, landing-next-tick,
spit-saves-tick, leaves-delay, odd-observer, comparator-odd, button-pulse, torch-delay,
center-exit, open-4 (+ open-4-cycle, open-4-close3), center-return, corner-return. The rest of the library
(40 older gadgets; `set`/`clear` replayed as the game's /setblock) was also run through `door
vanilla-gadget`: all identical, so the whole library matches the official server.

## Findings worth knowing
* A BUD-armed piston cannot be part of a rest state (the judge's placement updates every block):
  0-tick partners must be armed after the flip.
* A piston's quasi cell is the natural home of its 0-tick partner (the partner shoves the source
  next to that cell and, setting itself extended, updates the piston). Up-facing pistons have no
  usable quasi cell (it is their front). A down-facing puller whose quasi cell another piston
  holds (A' under F in open-4) is armed through that cell and 0-ticked by a partner beside it.
* The "silent arming" sources must not touch any other piston's quasi cell: every bug met while
  building open-4/corner-return was an arming block quasi-powering a neighbor (a puller that then
  never retracted, a parked piston that re-extended into the walkway).
* The update order of ONE change (W, E, D, U, N, S) is the cheapest way to order two pistons in
  the same phase: put the piston that must go first west of the redstone block.

## Not done / next
* Wiring open-4 into a lever-driven door (ticks.md section 6 has the input table: which input at
  which tick and what can provide it).
* Close 2: TL's chain padded by ~5 BUD pistons so TL's puller fires after T left its cell inside
  tick 2; close 1 needs C's first move and the carried pistons in the same tick too (relays).
