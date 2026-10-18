# minecraft-tas

Engineer Minecraft redstone against the **real game code**: a headless Minecraft 26.3 simulator
that runs the game's own server classes, one game tick at a time, hundreds of sandboxes per
machine, plus the tools built on it. Its first project: **the 3x3 piston door with the fewest
blocks**, built from first principles by a night of agents.

* **Real mechanics.** Nothing is re-implemented: pistons, redstone dust, observers and block update
  order are vanilla, and every record behaves identically, tick for tick, in the unmodified
  official server.
* **Tick by tick.** Nothing advances unless asked: place blocks, flip a lever, step one tick, read
  every block back.
* **Many at once.** Each simulation is an isolated sandbox (its own level, clock and random); one
  JVM hosts many sandboxes, a pool runs one JVM per core.
* **Readable source.** Minecraft ships unobfuscated since 26.1; `mctas src` decompiles it so
  mechanics are checked in the code instead of folklore.

## The smallest 3x3 piston door

A door's size is its **volume**, as the piston door community counts it (vjeux, 2026-10-18: the
bounding box of the build over every state of a cycle, the lever excluded; [the rules](docs/rules.md)).
The smallest solid door is **80b**: flat-b-v7, 8 x 10 x 1, 44 blocks placed.

Every solid record by blocks placed (the count before sizes were volumes), from 151 down to 38,
filmed by one camera:

[![every solid record](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.jpg)](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.mp4)

The best door of each board by size (state on 2026-10-18 at 20:35Z; the live rankings, each
leader with its GIF: [LEADERBOARD.md](designs/piston-door-3x3/LEADERBOARD.md)):

| Solid (main ranking), no slime, flat, in a 3-thick wall: 80b (8x10x1, 44 blocks placed) | Seamless (FULL, hallway-compatible): 100b (10x10x1, 57 placed) | Sculk: 252b (7x9x4, 36 placed) |
|---|---|---|
| ![flat-b-v7-44b](https://github.com/vjeux/minecraft-tas/releases/download/videos/flat-b-v7-44b.gif) | ![flat-v2-57b](https://github.com/vjeux/minecraft-tas/releases/download/videos/flat-v2-57b.gif) | ![sculk-ew-s4br-36b](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk-ew-s4br-36b.gif) |
| **Gravity (sand door): 126b (7x9x2, 33 placed)** | **Fastest solid: opens in 9 ticks, closes in 7 (720b)** | **Not rotation-safe: 252b (36 placed)** |
| ![gravity2-g33-33b](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity2-g33-33b.gif) | ![fast-clean-c1r-107b](https://github.com/vjeux/minecraft-tas/releases/download/videos/fast-clean-c1r-107b.gif) | ![sculk2-s4b-36b](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk2-s4b-36b.gif) |

The fewest blocks placed is still sf-holdA's b38 (38 blocks, size 252b: 7x9x4), and tick-door's
door opens in 4 ticks, the proven floor for a solid door. Every design counts only if
`mctas door verify <design> --extra-offsets 28` passes from main: 3 open/close cycles at 32 world
positions ([the rules](docs/rules.md)).

## Docs

| page | what is in it |
|---|---|
| [docs/rules.md](docs/rules.md) | the judge, the categories and what every board means |
| [docs/mechanics.md](docs/mechanics.md) | game mechanics read from the source, the timing of every component (0-tick, odd ticks, the fastest-door floors) and the gadget catalog |
| [docs/tools.md](docs/tools.md) | every `mctas` command and when to use it |
| [docs/internals.md](docs/internals.md) | how the simulator, the bridge protocol, videos, tick videos and world export work |
| [docs/history.md](docs/history.md) | what happened: record progressions, the door families and how they work, discoveries, dead ends |
| [LEADERBOARD.md](designs/piston-door-3x3/LEADERBOARD.md) | generated: every board with its leader's GIF |
| [archive/README.md](designs/piston-door-3x3/archive/README.md) | generated: every archived design with its blocks, ticks, buildability, video and how it works |
| [tracks/README.md](designs/piston-door-3x3/tracks/README.md) | the agent tracks, one line each; their notebooks are `tracks/<track>/NOTES.md` |
| [AGENTS.md](AGENTS.md) | how agents work in this repo |

## Quick start

```sh
cargo build --release
./target/release/mctas setup                 # Java 25, server jar + libs, compile the bridge
./target/release/mctas src decompile         # readable game source in ~/.cache/mctas
./target/release/mctas src show PistonBaseBlock --grep 'getNeighborSignal' -C 12
./target/release/mctas door verify designs/piston-door-3x3/archive/sf-holdA-b38-38b.txt --extra-offsets 28
./target/release/mctas door trace designs/piston-door-3x3/archive/fc2-40b.txt
```

Everything downloaded or generated (Java runtime, Minecraft jars, decompiled sources) lives in
`$MCTAS_HOME` (default `~/.cache/mctas`), never in this repository.
