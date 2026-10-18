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
| piston-door-3x3-v2-133b.txt | 133 | unified late chain, A torch fed off the sides line |
| piston-door-3x3-v2-128b.txt | 128 | P_T nets hang off the lever line, A torch fed by repeaters |
| piston-door-3x3-v2-126b.txt | 126 | greedy removal of 128 |

The 143- and 135-block intermediates of v2 were never committed and are lost.
