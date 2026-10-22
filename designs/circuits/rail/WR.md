# Study 19: minecart stations (WR board)

The basic block of rail transit: a cart coming down the line stops at the platform, and a button
press sends it on its way. Kind `station 1` (on a one-way line) and `station 2` (on a two-way line,
where carts coming the other way must pass through). clk-obs, coordinator 2026-10-21.

## The judge (`crate::station`, docs/circuits.md "station 1 | station 2")
- Ports: `port line.in:` and `port line.out:` (the first cell of the judge's own track at each end:
  air beside one of the design's end rails; the judge lays 8 cells of straight plain rail outward
  from each, on stone where the design has nothing below; the design must stand without it),
  `port input.go: x y z button` (the departure button, the input device: not counted).
- In the game only, with the game's own minecarts. A rider is an armor stand riding the cart
  (`/ride`), so the cart runs the ridden physics (`OldMinecartBehavior`: its move scaled by 0.75,
  slowed by 0.997 a tick instead of 0.96). redsim models empty carts but not passengers.
- One run, 17 carts one after the other, ridden and empty, sent in at the station's edge with 0.02
  to 2.0 blocks per tick of momentum (2.0: what a cart keeps off a booster line). Some cycles press
  the button with the station empty first (nothing may change); some press while the cart is still
  braking (it may leave at once or stop and wait for the next press, but must leave its way).
- PASS for every cart: at rest inside the station within 300 gt; a ridden one where the rider can
  click the button from the cart (a look ray from a player's eyes, 1.2075 above the cart, reaching
  the button within 4.5 blocks past every block in the way; the player's own cart does not take the
  click, as in the game); held still 5 to 154 gt (seeded) until the press; moving within 80 gt of
  the press, out through `line.out`, rolling 6 more blocks by itself; the rider still aboard.
  `station 2` also sends 8 carts the other way, fast enough to roll through that much plain track:
  each passes through without stopping. Robust = 4 positions x 4 rotations.
- Metrics: blocks (the button not counted), stop distance (the farthest a cart rests past the
  station's entry), departure delay (the longest from a press to the first move), leaves at (the
  slowest leaving cart where it leaves the station), other way passes at.

## Board
| design | line | blocks | box | stop distance | departure | status |
|---|---|---|---|---|---|---|
| [station-slope-v2](archive/station-slope-v2.txt) | one-way | 6 | 3x3x2 (18) | 1.60 | 1 gt, leaves at 0.22 b/t (empty) | robust (clk-obs, 2026-10-21) |
| [station-slope2-v4](archive/station-slope2-v4.txt) | two-way | 10 | 5x3x2 (30) | 1.60 | 1 gt, leaves at 0.37 b/t; the other way passes at 0.40 b/t | robust (clk-obs, 2026-10-21) |

## How they work
- **The stop.** Unpowered, a powered rail halves a cart's speed every tick (`moveAlongTrack`'s
  halt: under 0.03 it stops dead). A cart at 2.0 b/t needs about 1.6 blocks of such rail to stop,
  so two of them. A cart can only be launched where it rests on a SLOPE: a stopped cart on a flat
  powered rail is kicked only away from a solid block beside the rail on the line itself, which a
  through line cannot have; on a slope, gravity's 0.0078 per tick starts it and the powered rail
  boosts it from 0.01 b/t on. So the line drops two blocks through the station on two powered
  rails sloping down the way the carts go: every cart, slow or fast, ridden or empty, rests on one
  of the two (the slowest at the top edge, a 2.0 b/t one 1.6 blocks in).
- **The button.** A floor button on a stone beside the lower slope: it strongly powers the stone,
  which powers the lower slope, which passes the power up to the other slope (powered rails pass
  power along up to 8 connected powered rails). The button sits below a rider's eyes wherever the
  cart rests, so a look ray from the cart reaches it; a button on the side of a support under the
  track would be hidden behind that block (`station-slope-v1` in the track: the judge refuses it).
- **The other way (`station 2`).** A cart climbing the slopes the other way would be stopped by them,
  so a detector rail after the lower slope powers both while a cart is on it. A detector stays on
  until its first 20 gt re-check after the cart left, so a slow cart that spends most of those
  20 gt on the detector has too little power left to climb (`station-slope2-v2` in the track: a
  0.1 b/t ridden cart stopped halfway up). An always-on booster (a powered rail on a redstone block)
  before the detector speeds every such cart up first, so it crosses the detector in a few ticks
  and climbs while the slopes are still on. The detector rail between the booster and the slopes
  keeps the booster's power out of them, and the booster also pushes leaving carts out at speed.

## Why these sizes
- **One-way, 6 blocks, 3x3x2.** Two halting slope rails (a 2.0 b/t cart overshoots one: 1.08 blocks
  of halting rail on a slope), each on a support, the upper one's high side resting on a block as
  every ascending rail must (`BaseRailBlock.shouldBeRemoved`): 5 blocks, 3 long and 3 high. The
  button needs a block of its own: no face of those three supports is in sight from a cart resting
  on the slopes (their tops carry the rails or the line, their sides face along the track or
  away), and nothing may stand in the cart's path above the track, so the button's stone goes beside
  the track: the box is 2 deep.
- **Two-way, 10 blocks, 5x3x2.** The one-way station plus the detector rail on its support and the
  booster on its redstone block, both past the lower slope (the detector must be beside the lower
  slope, and the booster must come before it for a cart coming the other way).

## Hopper-minecart unloaders and loaders (kinds `cart-unloader`, `cart-loader`)

Storage transport: a hopper minecart full of items arrives at the unloader, stops, empties every
item into the chest and leaves on its own once empty; an empty hopper minecart arrives at the
loader, stops, fills from the supply and leaves on its own once full or once the supply runs dry.
Judge: `crate::cartio` (docs/circuits.md "cart-unloader | cart-loader"): 12 real carts a run (a
loader 13, the last after its supply waited 3000 gt), 0.02 to 2.0 b/t, mixed loads, full stacks,
16-stackables, non-stackables, empty carts or an empty supply; every item accounted for after
each cart; the audit's stray items on every open hopper top.

| design | kind | line | blocks | box | rate | departure | status |
|---|---|---|---|---|---|---|---|
| [cart-unloader-t2](archive/cart-unloader-t2.txt) | unloader | terminus | 25 | 5x3x3 | 2.47 items/s | 15 gt after empty | robust (clk-obs, 2026-10-21) |
| [cart-loader-t2](archive/cart-loader-t2.txt) | loader | terminus | 39 | 8x4x5 | 2.48 items/s | 12 gt after full or dry | robust (clk-obs, 2026-10-21) |

### How they work
- **Unloader.** The cart rolls in over a booster (every cart arrives fast, even a crawling one), a
  detector rail and a plain rail onto a powered rail over the hopper and stops against the end
  block. The powered rail is the departure: powered, it kicks the stopped cart away from the end
  block, back out. It is powered (a torch inverter, dust, a repeater into the end block) only when
  the hopper under the cart is empty AND the detector rail has been quiet for its 20 gt: an arriving
  cart parks (the detector holds the rail off until the hopper has its first item), a loaded cart
  waits while the hopper keeps getting items (a hopper that ejects and pulls in the same tick never
  reads empty in between), an empty cart or one just emptied is sent back. Why not simpler: a single
  torch from the comparator on that hopper cannot reach the rail (the comparator's output lands two
  cells from the hopper at its own level); and without the detector the rail is powered whenever no
  cart is there, so every arriving cart would bounce straight back.
- **Loader.** "Full" can only be read through a detector rail under the cart (its comparator reads
  the cart's fill), and a cart parked on a detector rail cannot be kicked away. So the cart parks on
  a halting powered SLOPE P just below a detector-rail slope D: boosters send every cart up P and D
  to the stop block, it rolls back and halts near P's top, where its box still reaches into D's
  search box. D stands on glass (a detector strongly powers the block under it; glass does not pass
  that on to P). The supply hopper beside P's top pushes into the cart from the side (a hopper above
  the slope would be in the cart's way down). P is powered (the cart leaves, back down the line) when
  the comparator on D reads 15 (a redstone block at its side: compare mode passes only 15 = every
  slot full) or when the supply hopper is empty (dust from its comparator keeps a torch off; the
  torch comes on when the supply runs dry). With no supply at all, P stays powered and an arriving
  cart rolls up, back and away at once.
