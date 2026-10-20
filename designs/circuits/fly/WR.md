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
| two-way | 14 blocks (6-block engine + two 4-block stations) | 14: stations of 3 blocks or fewer do not exist for any reversible 6-block engine (searched k <= 3), so 6 + 4 + 4 | RECORD = BOUND; keep the faster round trip (98 gt), fold the other |
| payload | 6 + n blocks (n <= 18: 9 per part, the 12-block push limit) | 6 + n is the bound (the engine plus the payload) | RECORD = BOUND; the card states "carries n blocks" |
| train | 5 + 3n blocks, no size limit | per trailer: a piston-free pulled part is 3 blocks | record |
| vertical two-way | none | 7-block engines fly both ways; the bottom station needs 4+ blocks | open |

Cards must say slime or honey, and a payload card how many blocks it carries.
