# How each design works, in one line

Read by `mctas door gallery` and `mctas door montage` (the caption under each GIF and clip).
A design missing here falls back to its row in the record progression of
[archive/README.md](archive/README.md). One row per archived design (`archive/<design>.txt`);
add a row when a new design tops a board.

| design | how it works |
|---|---|
| logic-min-slime-free-50b | agent logic-min, no slime or honey anywhere: each side column stands on 3 sticky pistons, retracted at rest and fired by 2-tick observer pulses (a short pulse pulls a block on open, pushes it back and lets go on close); the right observer fires straight into the right pistons' dust (on close the top piston and the left column move once more before settling) |
| st43-bmobs-42b | agent top-control on search-tool's 43: the redstone-block family with a sculk sensor and one repeater as the late hold, and the bottom-middle piston retracted at rest, fired by an observer watching the bottom-right door block |
| gravity-g6-49b | agent gravity, all 9 door blocks are sand: the side columns ride slime backbones pushed open by sticky pistons that are the doorway floor; the middle column stands on a 3-piston tower in a shaft under the door that collapses top-down (the sand drops 3 cells) and lifts it back bottom-up; g6 hangs the first torch under a dust (-2 from g4) |
| search-tool-from-fresh-b-gravity-g6-49b-46b | agent search-tool's local search on gravity's g6 (49): an obsidian block and a repeater became one observer, then removals (-3). The door itself is g6's: sand side columns on slime backbones pushed open by the doorway-floor pistons, the sand middle column dropping into a 3-piston shaft under the door and lifted back |
| gravity-g4-51b | agent gravity, the "drop tower": sand side columns pushed open by floor pistons, the sand middle column falls into a 3-piston shaft under the door; opens in 5 ticks |
| piston-door-3x3-rb8-71b | agent redstone-block, the rb family tuned for speed: a second observer on the lever moves the B window to ticks 4..12, and the hold's last repeater powers Q from behind so its falling edge updates Q at once (open 16, close 17) |
| piston-door-3x3-rb9-61b | agent redstone-block: rb8 through the speed-constrained minimizer (`door minimize --max-open 16 --max-close 17`): one repeater gone, another moved next to the observer's dust line; as fast as rb8 with 10 fewer blocks |
| piston-door-3x3-rb7-71b | agent redstone-block: first fast rb door, the B window moved from ticks 10..18 to 4..12 by a second observer watching the lever (open 16, close 19) |
| m48-bmobs-47b | agent top-control: the observer-fired bottom-middle piston (as in rb6-bmobs) on mechanics' rb5-hold (48) |
