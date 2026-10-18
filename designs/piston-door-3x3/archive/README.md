# Archive of 3x3 door designs

Every design that verified, one file each, named `<name>-<blocks>b.txt`. Each file starts with
its verify result and where it came from. The ranked list of everything (including the agents'
branches) is [../LEADERBOARD.md](../LEADERBOARD.md). New records are archived here with
`mctas door verify <design> --extra-offsets 28 --archive --archive-dir designs/piston-door-3x3/archive`.

## Record progression (category solid: 9 identical plain door blocks)

| blocks | file | how |
|---|---|---|
| 151 | piston-door-3x3-v2-151b.txt | first valid design: all-stone door, "UP dance" |
| 147 | piston-door-3x3-v2-147b.txt | greedy removal of 151 |
| 143 | piston-door-3x3-v2-143b.txt | A early through a 21-block route across the front, unified late chain |
| 135 | piston-door-3x3-v2-135b.txt | A early from a branch of the sides line |
| 133 | piston-door-3x3-v2-133b.txt | unified late chain, A torch fed off the sides line |
| 128 | piston-door-3x3-v2-128b.txt | P_T nets hang off the lever line, A torch fed by repeaters |
| 126 | piston-door-3x3-v2-126b.txt | greedy removal of 128 |
| 118 | piston-door-3x3-v2-118b.txt | R_B stretches the B pulse, straight late line, A late off the P_T net |
| 117 | piston-door-3x3-v2-117b.txt | greedy removal of 118 |
| 116 | piston-door-3x3-v2-116b.txt | router re-laid the drop to the sides line |
| 115 | piston-door-3x3-v2-115b.txt | greedy removal of 116 |
| 109 | search-tool-v2-min-109b.txt | overnight agent search-tool: local search + net re-synthesis of v2 |
| 96 | piston-door-3x3-v2u2-96b.txt | agent center-mech: side groups pushed out by 2 pistons sharing an obsidian block, honey carrier for the bottom-middle, redstone blocks riding the side groups |
| 91 | mechanics-ta-91b.txt | agent mechanics: A torch next to the observer column, lever on its attachment |
| 90 | mechanics-ta-90b.txt | agent mechanics: minimizer removed a stray floor |
| 88 | search-tool-center-mech-88b.txt | agent search-tool: local search on center-mech's 96 |
| 87 | piston-door-3x3-v3c-87b.txt | agent top-control: side pistons high, redstone blocks riding the backbones power A, P_BM, P_T |
| 73 | piston-door-3x3-rb1-73b.txt | agent redstone-block: sides pushed open, one redstone block shuttled by one piston powers A then P_R, the right structure drags T out (no P_T) |

Invalid (kept for history): piston-door-3x3-v1-invalid.txt (125) and -v1-min-invalid.txt (121)
use a slime block as the top-middle door block.
