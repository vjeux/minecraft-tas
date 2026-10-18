# Track `tick-study`: 0-tick and 1-tick gadgets, floors for the 3x3 door

The study is `docs/ticks.md` (catalog, components table, pulse generators, floors with proofs and
schedules). Gadgets in `designs/gadgets/` (rows in `docs/mechanics.md`), all passing `door drive`
and identical in the official server at 4 positions (`door vanilla-gadget`, logs/ here).

## Results
* Floors (solid): **open >= 4**, **close >= 1** (open + close >= 5). Boards: 10 / 7.
* **`open-4`**: a whole 3x3 opening clear at tick 4 (ideal inputs), the open floor. Critical path
  alone: `center-exit`.
* **Close 4 on the same machine** (`open-4-cycle.drive`): k1 C and T into the top-row cells,
  k2 C home (A' carried back into E fires as it lands), k3 T home (P_T carried back), k4 TL.
  Pieces: `center-return`, `corner-return`. Two carried-piston chains started at one tick boundary
  interleave in the FIFO block-event queue (a failed push never retries), so without same-tick
  relays it is one chain per tick; relays would give close 1-3.
* New tool: `mctas door vanilla-gadget <drive>...` (crates/piston-door/src/vanilla_gadget.rs) and
  the drive command `setblock` (the game's /setblock, identical in both worlds).

## Gadgets (21 new)
zero-tick, zero-tick-normal, zero-tick-reach, zero-tick-onplace, zero-tick-observer,
zero-tick-pulse, double-move, instant-off, instant-on, event-order, landing-next-tick,
spit-saves-tick, leaves-delay, odd-observer, comparator-odd, button-pulse, torch-delay,
center-exit, open-4 (+ open-4-cycle), center-return, corner-return. The rest of the library
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
* Wiring open-4 into a door (lever, instant-off line, arming after the flip, a k4 observer chain)
  and its close; the open-4 topology's own close is serialized (~8, ticks.md section 6).
* Close 1-2 end to end (relays instead of tick boundaries).
