# Study 7, flying machines: world-record targets (2026-10-20)

vjeux 12:45Z: world-record quality, quality over quantity. Bounds from the mechanics and the
census (tracks/fly/NOTES.md); a record is one card per (direction, speed, blocks, glue), the other
layouts listed on it, not 47 cards.

| variation | record now | bound / target | status |
|---|---|---|---|
| smallest, horizontal | 6 blocks, 10 gt/block (47 layouts) | 6 is the minimum: the census of every box up to 6 cells long finds no machine with 5 blocks or fewer (a machine needs two moving parts, each a piston + a glue block + a trigger) | RECORD = BOUND; fold to one card per glue (slime, honey) |
| fastest, horizontal | 7 blocks, 9 gt/block | 9 gt/block is the ceiling found (push 2 + trigger 1 + reach-pull 2 + pull 2 + landing 2); target: 6 blocks at 9 (the up machines do it with a redstone block for the rear observer) | search |
| up | 6 blocks, 9 gt/block | both bounds met | RECORD = BOUND; one card |
| down | 6 blocks, 10 gt/block (robust: 8 layouts) | target 6 blocks at 9 | search; fold to one card |
| with docking stations (two-way) + on/off lever | fly-docking-15b: 15 blocks (6-block engine + a 4-block station + a 5-block station whose observer a sticky piston moves; the lever is not counted), round trip 98 gt, docks within 87 gt of the lever going off, restarts at once | without a switch 14 was the bound (stations of 3 blocks or fewer do not exist for any reversible 6-block engine, searched k <= 3); the switch costs one sticky piston | THE design (vjeux 00:22Z asked for the on/off switch); judged by flying_switch: off at rest, four on/off cycles at different moments of the trip, it must dock at the same place every time and shuttle again |
| payload | 6 + n blocks (n <= 18: 9 per part, the 12-block push limit) | 6 + n is the bound (the engine plus the payload) | RECORD = BOUND; the card states "carries n blocks" |
| train | 5 + 3n blocks, no size limit | per trailer: a piston-free pulled part is 3 blocks | record |
| vertical two-way | none | 7-block engines fly both ways; the bottom station needs 4+ blocks | open |

Cards must say slime or honey, and a payload card how many blocks it carries.

## clk-item pass (2026-10-21)

- On/off switch on the docking-station machine: done, fly-docking-15b (see its row); new judge flying_switch.
- One-way below 6 blocks: the census behind the bound is exhaustive over every SHAPE a 5-block machine can have (every pentacube's bounding box — 5x1x1, 4x2x1, 3x3x1, 3x2x2, 2x2x2, upright or flat — fits one of its 14 boxes), with slime, honey, stone, observers, pistons, sticky pistons, and redstone blocks in the k = 6 pass. Its scope limit is the simulator: redsim, then every 6-block find re-judged in the game. A 5-block machine that only flies in the real game (an update-order or 0-tick effect redsim does not model) would be missed. That is the honest residue, not a known candidate.
- Payload: 6 + n blocks carrying n is the bound by definition (the cargo is counted); the cap is 9 per half (the 12-block push limit), 18 in all.
- Train: 3 blocks per trailer (slime + sticky piston + the observer that fires it after the trailer lands). A 2-block trailer needs that piston fired without its own observer: by the previous trailer's observer through a conductor, or by a redstone block riding along. Untried; this is the open item on this board.

## Real use (track fly, 2026-10-21: coordinator's real-use bar, with clk-misc's judge audit)

Every kind with a real switch, at minimum volume (box), game-robust (4 positions x 4 rotations):

| kind | record | box (volume) | how it starts / stops |
|---|---|---|---|
| flat (horizontal one-way) | fly-horizontal-start-7b-10gt: 7 blocks, 10 gt/block, flies along the ground | 3x4x1 (12) | start: flip the lever |
| up | fly-up-start-9b-9gt: 9 blocks, 9 gt/block | 4x3x1 (12) | start: flip the lever |
| down | fly-down-start-9b-10gt: 9 blocks, 10 gt/block | 2x2x3 (12) | start: flip the lever |
| payload | fly-payload-start-7b (base fly-payload-start-n1): carries n = 1..18 blocks, 7 + n blocks, 10 gt/block, flies along the ground; every member game-robust | 3 x (4 + n) x 1 up to 9 (n1: 15), then 3 x (4 + ceil(n/2)) x 2 (n18: 78) | start: flip the lever |
| docking stations | fly-docking-15b-78gt: 15 blocks, round trip 78 gt, needs 1 block of air | 4x2x8 (64; fly-docking-15b is 5x2x9 = 90) | on/off lever: off docks it at the lever's station |

- The start judge (check.rs `flying_start`; a lever on a machine without a kick, or a button): parked at rest, the first flip launches it, it flies its distance intact, and the station (lever + its stone, `port station`) ends as built. It is run once per flip gap of the audit (START_FLIPS 4, 5, 6, 8, 10, 12, 16, 20 gt: off k gt after the launch, on again, off again, then twice long after it left). A watcher left on the parked part fails it (selftest cal-fly-start-watcher-stays); every record's trigger observer rides the part the first push moves, so later flips reach nothing.
- The docking judge now also wants the shuttling to go on until the switch turns off (an annealed machine made one trip and parked with the lever on: selftest cal-fly-docking-parks).
- Ground clearance (robust::fly_clearance, every horizontal machine): the fewest air blocks over grass at which it still passes; slime or honey in the bottom layer drags the grass, and an observer in the bottom layer watching down sees the grass under it turn to dirt (a random tick), so both need a block of air.
- Tools: `circuit fly start-scan` (the device in the kick cell; `--watcher`: an observer firing a piston directly, stuck to the machine by up to 2 slimes), then `circuit compact` on the start judge. Smaller boxes than 12 were not found (compact from every 12-volume seed: no smaller; a watcher on a piston's back needs the piston-W-lever line plus the stone).
- Train: dropped from the real-use list (compact turned its seed into a plain 2-part machine with a passenger; the judge does not know trailers).
