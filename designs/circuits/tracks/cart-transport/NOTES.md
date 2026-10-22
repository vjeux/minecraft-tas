# Cart transport track (designs/briefs/cart-transport.md)

Judges:
- each station: the rail study's `mctas circuit cart-run` / `circuit verify` (12-13 carts, mixed
  loads, 0.02-2 b/t, strays);
- the whole line: `mctas circuit cart-system <line> [--sim]` (crate::cartsys: a double chest of
  3456 items through one cart over the line's track; every item must arrive).

`mctas circuit cart-line --loader A --unloader B --out line.txt` writes a line: B turned half a
turn at the west end, 128 blocks of track (a powered rail every 8, each lit by a torch), A at the
east end, aligned on both stations' `port line.in:` cells.

| design | what | blocks | judge |
|---|---|---|---|
| ld-chest.txt | loader: the cart parks under the supply chest (20 items/s pull) | 34 | 17.46 items/s; verify passes, "rotation-dependent" only by 17.46 vs 17.48 (the rate's one-tick rounding) |
| ul-gate.txt | unloader: the cart held off-centre against a fence gate, two hoppers drain it | 31 | 4.79 items/s, robust |
| line-site.txt | site t5 + t3 | 344 | 52.4 min a double chest, 1.10 items/s |
| line-a.txt | ld-chest + t3 (the obvious system) | 343 | 32.4 min, 1.78 items/s |
| line-b.txt | ld-chest + ul-gate | 353 | **21.0 min, 2.74 items/s** (95% of one cart's ceiling) |

## What the mechanics forced
- **Loader:** a chest over the climb catches the cart. Rolling back with its top above the
  chest's bottom, it stuck on the chest's face, so an open trapdoor stops the climb under the
  chest. When composing lines, the second chest half must stay off the rail row: with t5 it went
  over the rail and the cart stuck after one trip.
- **Unloader, why it drains with two hoppers:** a cart against a full end block is centred and
  overlaps one cell, so only one hopper reaches it. A closed fence gate's plate is in the middle
  of its cell, so the cart stops 0.115 into the next cell. On the slope, the cart overlaps the
  slope's cell, which the hopper below pulls from, and the gate's cell, which the hopper beside
  pulls from. That hopper is also the slope's support: a hopper's top rim is rigid enough to hold
  a rail.
- **Unloader timing, first problem:** the comparator → torch → repeater chain takes 6 gt. A
  cart that hits the gate starts rolling back two ticks later on a powered slope. A detector rail
  at the foot of the slope fixes it by cutting the power just in time.
- **Unloader timing, second problem:** the first version still failed, because a pressed detector
  rail next to the slope powers the slope itself. A plain rail between them fixed it.

## Open
- An empty cart keeps shuttling while the supply is dry: ld-chest sends a cart away from a dry
  chest, as its station judge requires.
- A plain terminus (the reviewer's ~12-15 block estimate) as the block comparison: not built.
