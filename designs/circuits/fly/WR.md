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
