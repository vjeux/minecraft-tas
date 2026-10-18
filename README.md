# minecraft-tas

Infrastructure to engineer Minecraft redstone and farm contraptions against the **real game
code**, headless, one game tick at a time, hundreds of simulations per machine.

* **Real mechanics.** The simulator runs Minecraft's own server classes (26.3). Nothing is
  re-implemented: pistons, redstone dust, observers and block update order are vanilla.
* **Tick by tick.** Nothing advances unless you ask: place blocks, flip a lever, step one tick,
  read every block back.
* **Many at once.** Each simulation is an isolated *sandbox* (its own Minecraft level, clock
  and random). One JVM hosts many sandboxes; a pool runs one JVM per core.
* **Readable source.** Minecraft ships unobfuscated since 26.1; `mctas src` decompiles it so
  mechanics can be checked in the code itself instead of folklore.

## Quick start

```sh
cargo build --release
./target/release/mctas setup                 # Java 25, server jar + libs, compile the bridge
./target/release/mctas src decompile         # readable game source in ~/.cache/mctas
./target/release/mctas src grep 'class PistonBaseBlock'
./target/release/mctas src show PistonBaseBlock --grep 'getNeighborSignal' -C 12
./target/release/mctas bench                 # throughput / memory with many sandboxes
./target/release/mctas door verify designs/piston-door-3x3/<design>.txt --timeline
```

Everything downloaded or generated (Java runtime, Minecraft jars, decompiled sources) lives in
`$MCTAS_HOME` (default `~/.cache/mctas`), never in this repository.

## Layout

| Path | What |
|---|---|
| `bridge/` | The Java side: a headless `MinecraftServer` subclass + a JSON-lines protocol (compiled with plain `javac`, no mod loader). |
| `crates/mctas` | Rust library: setup, source reading, bridge build, worker/sandbox API, design format. |
| `crates/mctas-cli` | The `mctas` command. |
| `crates/piston-door` | 3x3 piston door project: spec, verifier, minimizer. |
| `designs/` | Contraptions in a human-readable layer format. |
| `docs/` | Architecture, protocol, and mechanics notes taken from the source. |

See [docs/architecture.md](docs/architecture.md) and [docs/protocol.md](docs/protocol.md).

## 3x3 piston door

Every solid record from 151 blocks down, filmed by one camera (3:11):

[![every solid record](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.jpg)](https://github.com/vjeux/minecraft-tas/releases/download/videos/solid-progression.mp4)

On the night of 2026-10-18, about 20 agents took the record from 115 to 40 blocks:
[docs/overnight-2026-10-18.md](docs/overnight-2026-10-18.md). The best door of each leaderboard
as a GIF: [designs/piston-door-3x3/GALLERY.md](designs/piston-door-3x3/GALLERY.md).

Current best (40 blocks), rendered by the real Minecraft client with `mctas door video`:
https://github.com/vjeux/minecraft-tas/releases/download/videos/fc2-40b.mp4

The ranked list of every design (main and the agents' `explore/*` branches) is in
[designs/piston-door-3x3/LEADERBOARD.md](designs/piston-door-3x3/LEADERBOARD.md). The best is
[designs/piston-door-3x3/archive/fc2-40b.txt](designs/piston-door-3x3/archive/fc2-40b.txt).
All nine door blocks are plain stone, 40 blocks in total, verified for 3 open/close cycles at 32
positions, and it behaves identically tick for tick in the unmodified vanilla server. (v1 used a
slime block as the top-middle door block, which is not a real door; the verifier now rejects it.)
See [docs/piston-door-3x3.md](docs/piston-door-3x3.md) for the rules, the boards and how fc2
works, and [docs/mechanics.md](docs/mechanics.md) for the mechanics read from the source.

## Tools

All tools are `mctas` subcommands, written in Rust; `--help` lists each one's flags.

* `setup`: install Java 25, the server jar and libraries, and build the bridge.
* `src decompile|grep|find|show|path`: read the decompiled 26.3 source.
* `sim repl|place`: talk to a simulator (JSON lines), or place a design and show what is pending.
* `bench`: measure throughput and memory with many workers and sandboxes.

The 3x3 door (`mctas door ...`):

* `verify`: the judge (3 cycles at 4 positions, 32 with `--extra-offsets 28`); prints blocks and ticks.
* `leaderboard`: verify every design on main and `explore/*`, then write LEADERBOARD.md.
* `vanilla-check`: replay designs in the unmodified official server and compare tick for tick.
* `reuse-check`: verify a design many times in one sandbox, to catch state leaking across runs.
* `gen` / `patch`: print a built-in design / apply an edit script (`set`, `clear`, `port`).
* `drive`: scripted inputs, probes and `expect` checks, a plane per tick; runs the gadget tests.
* `trace`: print every block change, tick by tick, after each lever flip.
* `ideal`: judge a mechanism driven by scheduled ideal inputs and sweep its timing windows.
* `route`: dust auto-router (fewest blocks; avoids swept cells and the walkway).
* `place`: try every combination of alternative patch groups (wiring puzzles).
* `minimize`: local search for fewer blocks or ticks; only the full judge accepts a candidate.
* `harvest`: run that search on the best designs of every branch and board.
* `grow` / `regrow`: add the cheapest 1-2 blocks / rebuild 2-3 control blocks with one fewer.
* `synth`: try every circuit of up to k blocks between two cells; judge each with the door or a spec.
* `video` / `video-all` / `card`: record one design / a whole folder / draw just the title card.
* `gallery` / `montage`: a GIF of each board's best (GALLERY.md) / a board's record progression.
