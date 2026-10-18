# Gallery: the best 3x3 piston door of each board

Looping GIFs rendered by the real Minecraft client (`mctas door gallery`, 1.5x the recording's 10 ticks/s). Rankings: [LEADERBOARD.md](LEADERBOARD.md). Every archived design has its own video: [archive/VIDEOS.md](archive/VIDEOS.md).

The whole solid record progression in one video (151 -> 40 blocks, `mctas door montage`; click the poster):

[![every solid record](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.jpg)](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.mp4)

The night, board by board (records from the archive):

![records over the night](https://github.com/vjeux/minecraft-tas/releases/download/videos/night-timeline.png)

## Solid (main ranking): 40 blocks

9 identical plain door blocks, no sculk sensors.

![fc2-40b](https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.gif)

**40 blocks**, solid, opens in 24 ticks, closes in 9 (worst cycle): [fc2-40b](archive/fc2-40b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.mp4)). Agent fresh-c, "symmetric control": open and close run the SAME signal chain at the same delays, so there is no hold at all: an observer on the lever pulses the center piston A 2 ticks after each flip (swallowed on open, it pushes the center down on close), the shuttle piston Q is the lever through one repeater (its redstone block powers A at rest, then the right piston, which drags the top-middle block out), and the B window fires on both edges but is harmlessly blocked on close; fc2 makes the left pusher a pull-type pulse piston west of its backbone, fired by two observers off the lever (opens in 24 ticks, closes in 9).

## Solid, no slime or honey: 48 blocks

Not a single slime or honey block anywhere.

![slime-free-combo-48b](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-combo-48b.gif)

**48 blocks**, solid, slime-free, opens in 24 ticks, closes in 50 (worst cycle): [slime-free-combo-48b](archive/slime-free-combo-48b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-combo-48b.mp4)). Agent slime-free, no slime or honey anywhere: logic-min's slime-free 50 (each side column on sticky pistons retracted at rest, fired by observer pulses; the right column fed by an observer + dust; a torch quasi-powers the top piston) combined with this track's left side, where the top-left door block goes UP into a puller fired by an observer chain, and a B-chain link re-grown by `door grow`.

Every record of this board in one video: [slime-free-progression.mp4](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-progression.mp4) ([poster](https://github.com/vjeux/minecraft-tas/releases/download/videos/slime-free-progression.jpg)).

## Sculk: 42 blocks

Uses sculk sensors (footsteps or mobs nearby can trigger them).

![st43-bmobs-42b](https://github.com/vjeux/minecraft-tas/releases/download/videos/st43-bmobs-42b.gif)

**42 blocks**, sculk, opens in 22 ticks, closes in 38 (worst cycle): [st43-bmobs-42b](archive/st43-bmobs-42b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/st43-bmobs-42b.mp4)). Agent top-control on search-tool's 43: the redstone-block family with a sculk sensor and one repeater as the late hold, and the bottom-middle piston retracted at rest, fired by an observer watching the bottom-right door block.

Every record of this board in one video: [sculk-progression.mp4](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk-progression.mp4) ([poster](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk-progression.jpg)).

## Gravity: 45 blocks

The 9 door blocks are sand, gravel or concrete powder.

![gravity-g10-45b](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-g10-45b.gif)

**45 blocks**, gravity, opens in 5 ticks, closes in 19 (worst cycle): [gravity-g10-45b](archive/gravity-g10-45b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-g10-45b.mp4)). Agent gravity, all 9 door blocks are sand: side columns on slime backbones pushed open by the doorway-floor pistons; the middle column stands on a 3-piston tower in a shaft under the door. Since g8 the middle piston S2 is retracted at rest and fired by an observer that rides the slime chain (so the close sequences itself), and g10 feeds the bottom piston's repeater from the floor of an existing dust.

Every record of this board in one video: [gravity-progression.mp4](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-progression.mp4) ([poster](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity-progression.jpg)).

## Fastest solid: 122 blocks

Fewest game ticks to open plus close (worst cycle), ties by blocks.

![speed-wire-w5-122b](https://github.com/vjeux/minecraft-tas/releases/download/videos/speed-wire-w5-122b.gif)

**122 blocks**, solid, slime-free, opens in 12 ticks, closes in 11 (worst cycle): [speed-wire-w5-122b](archive/speed-wire-w5-122b.txt) ([video](https://github.com/vjeux/minecraft-tas/releases/download/videos/speed-wire-w5-122b.mp4)). From agent speed; see [tracks/speed/NOTES.md](tracks/speed/NOTES.md).

