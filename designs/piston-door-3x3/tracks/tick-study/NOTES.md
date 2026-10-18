# Track `tick-study`: 0-tick and 1-tick gadgets, floors for the 3x3 door

The study is `docs/ticks.md` (catalog, components table, pulse generators, floors with proofs and
schedules). Gadgets in `designs/gadgets/` (rows in `docs/mechanics.md`), all passing `door drive`
and identical in the official server (`door vanilla-gadget`, logs/ here).

## Results
* Floors (solid): **open >= 4**, **close >= 1** (open + close >= 5). Boards: 10 / 7.
* Critical path of open 4 built and vanilla-checked: `center-exit` (center out of its cell at k1,
  out of the opening at k4).
* New tool: `mctas door vanilla-gadget <drive>...` (crates/piston-door/src/vanilla_gadget.rs) and
  the drive command `setblock` (the game's /setblock, identical in both worlds).

## Gadgets (16 new)
zero-tick, zero-tick-normal, zero-tick-reach, zero-tick-onplace, zero-tick-observer, double-move,
instant-off, instant-on, event-order, landing-next-tick, spit-saves-tick, leaves-delay,
odd-observer, button-pulse, center-exit.

## Findings worth knowing
* A BUD-armed piston cannot be part of a rest state (the judge's placement updates every block):
  0-tick partners must be armed after the flip.
* A piston's quasi cell is the natural home of its 0-tick partner (the partner shoves the source
  next to that cell and, setting itself extended, updates the piston). Up-facing pistons have no
  usable quasi cell (it is their front), and a down-facing puller whose quasi cell another piston
  needs (A' under F) must be armed and 0-ticked from the side.

## Not done / next
* Open-4 as a whole mechanism (ideal inputs) and as a wired door: schedule in ticks.md section 6.
* Close 1-2 end to end (center return with an instantly carried piston + relay).
