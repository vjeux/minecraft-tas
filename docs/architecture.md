# Architecture

```
 Rust (crates/mctas)                      JVM (bridge/, one per worker)
 ┌──────────────────────┐  JSON lines    ┌─────────────────────────────────────────┐
 │ Pool ── Worker ──────┼──stdin/stdout──┼─▶ Protocol (reader thread → queue)      │
 │          Worker ─────┼──────────────▶ │   SimServer extends MinecraftServer     │
 │          ...         │                │     ├─ overworld (void, never ticks)    │
 └──────────────────────┘                │     ├─ Sandbox 1 = its own ServerLevel  │
                                         │     ├─ Sandbox 2 = its own ServerLevel  │
                                         │     └─ ...                              │
                                         └─────────────────────────────────────────┘
```

## Why this shape

* **Real game code, no mod loader.** Minecraft 26.1+ ships unobfuscated, so the bridge is four
  plain Java files compiled with `javac` against the unbundled `server.jar` (`mctas setup`
  does it; the Rust crate embeds the sources). `SimServer` is modeled on vanilla's own
  `GameTestServer`: no network listener, no players, no autosave.
* **A sandbox is a `ServerLevel`.** Each one has its own block-tick queue, block-event queue,
  entity list, game time (its own `PrimaryLevelData`, `tickTime=true`) and seeded random. So
  sandboxes never interact, and determinism does not depend on what else runs in the JVM.
  Levels are registered in `MinecraftServer.levels` so code that looks levels up still works.
* **Nothing ticks by itself.** `tickServer` is replaced by the protocol loop: the server thread
  waits for requests and only `step` advances a sandbox. One `step` = vanilla
  `ServerLevel.tick()` (world border, weather, time, scheduled block/fluid ticks, raids,
  chunk/random ticks, block events, entities, block entities) followed by the server tasks a
  real server would run before the next tick.
* **Scale = many JVMs × many sandboxes.** The server thread is single-threaded and redstone
  code keeps per-block-type mutable state (e.g. `RedstoneWireBlock.shouldSignal`), so a JVM
  never ticks two sandboxes concurrently. Parallelism comes from running one JVM per core
  (`Pool`), each hosting several sandboxes; `-XX:ActiveProcessorCount=2` and SerialGC keep
  each JVM small.

## Things the source taught us (see docs/mechanics.md)

* Block entities (moving pistons) and scheduled block ticks only run in chunks whose entity
  sections are loaded, and that load is asynchronous: `Sandbox.create` pumps it before
  returning, otherwise the first ticks silently skip pistons.
* Vanilla runs queued server tasks only while it "has time" before the next 50 ms tick; with
  no wall clock, `SimServer.shouldRun` returns true or the entity-section loads never run.

## Videos

`mctas door video` runs the real 26.3 client (renderer "renderpearl"; falls back from OpenGL to
Vulkan on Mesa llvmpipe) on an Xvfb display. A worker started with `--listen <port>` accepts it
as a spectator; `play` steps the sandbox at a fixed ticks-per-second while serving the
connection, and ffmpeg records the display. The client is unmodified except for a launcher
class that hides the HUD.

The camera is the spectator's pose (`camera` teleports every viewer). For a moving camera the
recording loop in `piston-door/src/video.rs` ticks one tick at a time (`play 1`) on a
wall-clock schedule and, between ticks, re-poses the camera about every 25 ms (`camera` +
`hold`): an absolute teleport snaps the client's position (no interpolation), so the motion is
as smooth as the teleport rate. Title cards and labels are drawn in Rust with the client jar's
bitmap font (`mctas::mcfont`) and overlaid by ffmpeg: the static ffmpeg build has no
`drawtext` (ffmpeg 7 needs harfbuzz for it). Xvfb is stopped with SIGTERM so it removes its
`/tmp/.X<n>-lock`; locks of dead servers are reclaimed.
