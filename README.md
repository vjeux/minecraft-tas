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

Every solid record from 151 blocks down to 38, filmed by one camera:

[![every solid record](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.jpg)](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.mp4)

The best door of each board (state on 2026-10-18; the live rankings, each leader with its GIF:
[LEADERBOARD.md](designs/piston-door-3x3/LEADERBOARD.md)):

| Solid (main ranking): 38 blocks | Solid, flat (one block thick), no slime: 43 | Sculk: 42 |
|---|---|---|
| ![sf-holdA-b38-38b](https://github.com/vjeux/minecraft-tas/releases/download/videos/sf-holdA-b38-38b.gif) | ![lamp-sweep-flat43-43b](https://github.com/vjeux/minecraft-tas/releases/download/videos/lamp-sweep-flat43-43b.gif) | ![st43-bmobs-42b](https://github.com/vjeux/minecraft-tas/releases/download/videos/st43-bmobs-42b.gif) |
| **Gravity (sand door): 33 blocks** | **Fastest solid: opens in 10 ticks, closes in 7** | **Not rotation-safe: 36** |
| ![gravity2-g33-33b](https://github.com/vjeux/minecraft-tas/releases/download/videos/gravity2-g33-33b.gif) | ![lamp-sweep-a1-107-107b](https://github.com/vjeux/minecraft-tas/releases/download/videos/lamp-sweep-a1-107-107b.gif) | ![sculk2-s4b-36b](https://github.com/vjeux/minecraft-tas/releases/download/videos/sculk2-s4b-36b.gif) |

The 38-block solid door (sf-holdA's b38) also leads the "no slime or honey" board. Every design
counts only if `mctas door verify <design> --extra-offsets 28` passes from main: 3 open/close
cycles at 32 world positions ([the rules](docs/rules.md)).

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
