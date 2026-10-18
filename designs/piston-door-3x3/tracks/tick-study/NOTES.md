# Track `tick-study`: 0-tick and 1-tick gadgets, floors for the 3x3 door

The study is `docs/ticks.md` (catalog, components table, pulse generators, floors with proofs and
schedules). Gadgets in `designs/gadgets/` (rows in `docs/mechanics.md`), all passing `door drive`
and identical in the official server at 4 positions (`door vanilla-gadget`, logs/ here).

## Results
* Floors (solid): **open >= 4**, **close >= 1** (open + close >= 5). Boards: 10 / 7.
* **`open-4`**: a whole 3x3 opening clear at tick 4 (ideal inputs), the open floor. Critical path
  alone: `center-exit`.
* **Close 3 piecewise** (tick-boundary ordering): `center-return` (C placed at k2 after two
  placed moves, a piston carried into E fires the tick it lands) + `corner-return` (T placed at
  k2 through its corner, the corner block at k3); every other block is a 0-tick at k1. Relays
  (instant-on) instead of tick boundaries would give close 1.
* New tool: `mctas door vanilla-gadget <drive>...` (crates/piston-door/src/vanilla_gadget.rs) and
  the drive command `setblock` (the game's /setblock, identical in both worlds).

## Gadgets (18 new)
zero-tick, zero-tick-normal, zero-tick-reach, zero-tick-onplace, zero-tick-observer, double-move,
instant-off, instant-on, event-order, landing-next-tick, spit-saves-tick, leaves-delay,
odd-observer, button-pulse, center-exit, open-4, center-return, corner-return.

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
