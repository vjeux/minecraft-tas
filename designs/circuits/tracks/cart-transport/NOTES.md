# Cart transport track (designs/briefs/cart-transport.md)

Judges:
- each station: the rail study's `mctas circuit cart-run` / `circuit verify` (12-13 carts, mixed
  loads, 0.02-2 b/t, strays; a loader's empty cart at a dry supply must wait);
- the whole line: `mctas circuit cart-system <line> [--sim] [--trickle]` (crate::cartsys: a double
  chest of 3456 items through one cart over the line's track, every item must arrive; `--trickle`:
  300 items fed slowly, then nothing, and the cart must end parked at the loader).

`mctas circuit cart-line --loader A --unloader B --out line.txt` writes a line: B turned half a
turn at the west end, 128 blocks of track (a powered rail every 8, each lit by a torch), A at the
east end, aligned on both stations' `port line.in:` cells.

Archived (designs/circuits/rail/archive/): ld-chest.txt, ul-gate.txt; the system design is
designs/circuits/rail/system/line-b.txt.

| design | what | blocks | judge |
|---|---|---|---|
| ld-chest | loader: the cart parks under the supply chest (20 items/s pull); leaves when full, or when the chest is empty and it holds something; an empty cart at an empty chest waits | 44 | 16.87 items/s; game verify passes in all 16 builds, "position-dependent" only by the rate (see the brief) |
| ul-gate | unloader: the cart held off-centre against a fence gate, two hoppers drain it | 31 | 4.79 items/s, robust |
| line-site | site t5 + t3 (t5 left the archive) | 344 | 52.4 min a double chest, 1.10 items/s |
| line-a | ld-chest + t3 (the obvious system) | 353 | 32.4 min, 1.78 items/s; trickle: parks |
| rail/system/line-b | ld-chest + ul-gate | 363 | **20.9 min, 2.75 items/s**; trickle: all delivered by 5.9 min, the cart parked at the loader |
| calibration/line-shuttles | the loader before the dry-supply fix + ul-gate | 353 | trickle FAILS: 10 of 300 delivered in 115 min |

## What the mechanics forced
- **Loader, the chest over the climb:** a chest over the climb catches the cart. Rolling back with
  its top above the chest's bottom, it stuck on the chest's face, so an open trapdoor stops the
  climb under the chest. A full block there instead lets the cart wedge past the chest. When
  composing lines, the second chest half must stay off the rail row: over the rail it stopped the
  cart after one trip.
- **Loader, the dry-supply fix:** one compare-mode comparator on the detector rail, its side input
  "the chest has items" at 15 (the chest's comparator through a repeater: a double chest with a
  few items reads only 1). It replaces the old pair: a full comparator, plus a torch that launched
  any cart once the chest was empty.
- **Loader, the extra parts:** a repeater after the comparator, because a one-item cart reads
  fill 1 and dust lost it before the slope. A fourth booster, because a cart entering at 0.05 b/t
  halted on the slope 0.6 block up, out of the detector rail's view, and with the fix it would
  have waited forever.
- **Unloader, why it drains with two hoppers:** a cart against a full end block is centred and
  overlaps one cell, so only one hopper reaches it. A closed fence gate's plate is in the middle
  of its cell, so the cart stops 0.115 into the next cell. On the slope, the cart overlaps the
  slope's cell, which the hopper below pulls from, and the gate's cell, which the hopper beside
  pulls from. That hopper is also the slope's support: a hopper's top rim is rigid enough to hold
  a rail.
- **Unloader timing, first problem:** the comparator → torch → repeater chain takes 6 gt. A cart
  that hits the gate starts rolling back two ticks later on a powered slope. A detector rail at the
  foot of the slope fixes it by cutting the power just in time.
- **Unloader timing, second problem:** the first version still failed, because a pressed detector
  rail next to the slope powers the slope itself. A plain rail between them fixed it.

## Open
- A plain terminus (the reviewer's ~12-15 block estimate) as the block comparison: not built.
