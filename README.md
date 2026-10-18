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
