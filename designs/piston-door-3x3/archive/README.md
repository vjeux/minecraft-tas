# Archive of 3x3 door designs

Every design that verified, one file each, named `<name>-<blocks>b.txt`. Each file starts with
its verify result and where it came from. `mctas door verify <design> --archive` adds new ones
automatically (and skips a design that is already here).

| file | blocks | notes |
|---|---|---|
| piston-door-3x3-v1-invalid.txt | 125 | slime block as the top-middle door block (fails the 9-identical-blocks rule) |
| piston-door-3x3-v1-min-invalid.txt | 121 | same, after greedy removal |
| piston-door-3x3-v2-151b.txt | 151 | first valid design: all-stone door, "UP dance" |
| piston-door-3x3-v2-147b.txt | 147 | greedy removal of 151 |
| piston-door-3x3-v2-143b.txt | 143 | A early through a 21-block route across the front, unified late chain |
| piston-door-3x3-v2-135b.txt | 135 | A early from a branch of the sides line |
| piston-door-3x3-v2-133b.txt | 133 | unified late chain, A torch fed off the sides line |
| piston-door-3x3-v2-128b.txt | 128 | P_T nets hang off the lever line, A torch fed by repeaters |
| piston-door-3x3-v2-126b.txt | 126 | greedy removal of 128 |
| piston-door-3x3-v2-118b.txt | 118 | R_B stretches the B pulse, straight late line, A late off the P_T net |
| piston-door-3x3-v2-117b.txt | 117 | greedy removal of 118 |
| piston-door-3x3-v2-116b.txt | 116 | router re-laid the drop to the sides line |
| piston-door-3x3-v2-115b.txt | 115 | greedy removal of 116 |

The 143- and 135-block steps were rebuilt by replaying the session's edits on top of the 133
commit (`crates/piston-door/src/designs_history.rs`, `mctas door gen v2-143`); the replayed 133
matches the archived one exactly.
