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
| 88 | search-tool-center-mech-88b.txt | agent search-tool: local search on center-mech's 96 |
| 87 | piston-door-3x3-v3c-87b.txt | agent top-control: side pistons high, redstone blocks riding the backbones power A, P_BM, P_T |
| 73 | piston-door-3x3-rb1-73b.txt | agent redstone-block: sides pushed open, one redstone block shuttled by one piston powers A then P_R, the right structure drags T out (no P_T) |
| 57 | piston-door-3x3-rb2-57b.txt | agent redstone-block: rb1 with short nets (dual-edge observer next to the lever feeds R_B by 3 dust, the 24gt late hold fed straight from the lever) |
| 53 | mechanics-rb2-hold-53b.txt | agent mechanics: rb2 with the 24gt late hold re-laid as a short U west of the lever, glazed terracotta floors beside the slime backbone |
| 51 | mechanics-rb2-hold-51b.txt | agent mechanics: the 53 plus O2's pulse to R_B through a strongly powered block and an isolated cross dust |
| 50 | mechanics-rb5-bchain-50b.txt | agent mechanics: redstone-block's rb5 (BUD hold for Q, 52) + the O2 to R_B pulse bridge |
| 48 | mechanics-rb5-hold-48b.txt | agents mechanics + redstone-block: the BUD silent hold shrunk to 7 blocks + the O2 to R_B pulse bridge (both reached 48; also piston-door-3x3-rb6-48b.txt) |
| 47 | rb6-bmobs-47b.txt | agent top-control: rb6 with the bottom-middle piston retracted at rest, fired by an observer watching the bottom-right door block (pull on open, push+spit on close) |
| 46 | piston-door-3x3-fresh-b-mp6-46b.txt | agent fresh-b: "mirrored pump" family (bottom row on a slime row, pump below the door, motion-triggered flags), open 16 / close 16 |
| 42 | fc1-42b.txt | agent fresh-c: "symmetric control" family: no hold at all (Q = lever through one repeater), an early A pulse spits the center down on close, the B window is harmlessly blocked on close |
| 40 | fc2-40b.txt | agent fresh-c: fc1 with the left pusher as a pull-type pulse piston west of the backbone fired by two observers off the lever |
| 39 | sf-holdA-b39-39b.txt | agent sf-holdA (also slime-free): mech-synth's 19-block mechanism; both top corners pulled up and fed X's 3 pulses (an odd pulse count toggles a pulse-driven piston once, so the doorway bounces before it settles), the B window and the T piston on one observer chain |
| 38 | sf-holdA-b38-38b.txt | agent sf-holdA (also slime-free): b39 with the lever on the observer O; it weakly powers a note block that O_a watches (a zero-delay relay that keeps power out of the T piston's quasi cell) |

## Other notable designs (not records when found)

| blocks | file | note |
|---|---|---|
| 90 | mechanics-ta-90b.txt | agent mechanics: minimizer removed a stray floor from its 91 (the 88 had landed first) |
| 52 | piston-door-3x3-rb5-52b.txt | agent redstone-block: rb2 with a silent BUD hold for Q instead of the 24gt chain |
| 45 | piston-door-3x3-fresh-b-mp7-45b.txt | agent fresh-b: mirrored pump with a shorter pump path, open 16 / close 16 |
| 43 | piston-door-3x3-fresh-b-mp8-43b.txt | agent fresh-b: mirrored pump, fastest small solid door (open 16 / close 16) |
| 41 | fc2f-41b.txt | agent fresh-c: fc2 with the B window fed straight off the lever, open 22 / close 9 |

Every other board (slime-free, sculk, gravity, speed) is ranked in [../LEADERBOARD.md](../LEADERBOARD.md).

Invalid (kept for history): piston-door-3x3-v1-invalid.txt (125) and -v1-min-invalid.txt (121)
use a slime block as the top-middle door block.
