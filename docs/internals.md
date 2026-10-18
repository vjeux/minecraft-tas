# Internals: the simulator, the bridge, videos and world export

How the pieces work. What the tools do is in [tools.md](tools.md); the game mechanics they rely
on are in [mechanics.md](mechanics.md).

## Layout

| path | what |
|---|---|
| `bridge/` | the Java side: a headless `MinecraftServer` subclass and a JSON-lines protocol (compiled with plain `javac`, no mod loader) |
| `crates/mctas` | Rust library: setup, source reading, bridge build, worker/sandbox API, design format, the static ffmpeg, the client font |
| `crates/mctas-cli` | the `mctas` command |
| `crates/piston-door` | the 3x3 door project: judge, boards, searches, synthesis, videos, vanilla cross-checks |
| `designs/` | contraptions in the layer text format; `designs/gadgets/` (proven circuits), `designs/piston-door-3x3/` (doors, the archive, the agents' tracks) |

Everything downloaded or generated (Java runtime, Minecraft jars, decompiled sources, worlds)
lives in `$MCTAS_HOME` (default `~/.cache/mctas`), never in this repository.

## The simulator

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

* **Real game code, no mod loader.** Minecraft 26.1+ ships unobfuscated, so the bridge is a few
  plain Java files compiled with `javac` against the unbundled `server.jar` (`mctas setup` does it;
  the Rust crate embeds the sources). `SimServer` is modeled on vanilla's own `GameTestServer`: no
  network listener, no players, no autosave.
* **A sandbox is a `ServerLevel`.** Each one has its own block-tick queue, block-event queue,
  entity list, game time (its own `PrimaryLevelData`, `tickTime=true`) and seeded random. So
  sandboxes never interact, and determinism does not depend on what else runs in the JVM. Levels
  are registered in `MinecraftServer.levels` so code that looks levels up still works.
* **Nothing ticks by itself.** `tickServer` is replaced by the protocol loop: the server thread
  waits for requests and only `step` advances a sandbox. One `step` = vanilla `ServerLevel.tick()`
  (world border, weather, time, scheduled block/fluid ticks, raids, chunk/random ticks, block
  events, entities, block entities) followed by the server tasks a real server would run before
  the next tick.
* **Scale = many JVMs x many sandboxes.** The server thread is single-threaded and redstone code
  keeps per-block-type mutable state (e.g. `RedstoneWireBlock.shouldSignal`), so a JVM never ticks
  two sandboxes concurrently. Parallelism comes from running one JVM per core (`Pool`), each
  hosting several sandboxes; `-XX:ActiveProcessorCount=2` and SerialGC keep each JVM small.
  `MCTAS_MAX_WORKERS` caps the JVMs of every pool on a shared box.

Things the source taught us:

* Block entities (moving pistons) and scheduled block ticks only run in chunks whose entity
  sections are loaded, and that load is asynchronous: `Sandbox.create` pumps it before returning,
  otherwise the first ticks silently skip pistons.
* Vanilla runs queued server tasks only while it "has time" before the next 50 ms tick; with no
  wall clock, `SimServer.shouldRun` returns true or the entity-section loads never run.
* A sandbox that is cleared and reused must not keep state: redstone torch burnout memory once
  leaked across runs (`door reuse-check` catches it; every design was re-judged after the fix).

## Bridge protocol

One JSON object per line on the worker's stdin; one response line per request on stdout, in
order, echoing `id`. Errors: `{"id":..,"ok":false,"error":"..."}`. On startup the worker prints
`{"event":"ready","version":"26.3"}`. Positions are `[x,y,z]`.

| op | request fields | response |
|---|---|---|
| `ping` | | `{}` |
| `info` | | `version`, `sandboxes`, `heap_used`, `heap_max` |
| `create` | `chunks: [minCX,minCZ,maxCX,maxCZ]` (default `[-1,-1,0,0]`), `seed` | `sandbox`, `min`, `max` |
| `clear` | `sandbox` | back to empty void, game time 0, reseeded |
| `drop` | `sandbox` | destroys the level |
| `set` | `sandbox`, `blocks: [[x,y,z,"state"],...]`, `mode: "raw"\|"update"\|<flags>` | `changed` |
| `fill` | `sandbox`, `from`, `to`, `state`, `mode` | `changed` |
| `get` | `sandbox`, `from`, `to`, `moving?` | `from`, `size`, `palette`, `flags`, `data` (base64 u16 LE, x fastest, then z, then y), `time`, `moving?` |
| `use` | `sandbox`, `pos`, `face?` | `result` (levers/buttons toggled directly; other blocks get a mock player's right click) |
| `step` | `sandbox` or `sandboxes`, `ticks`, `until_idle?`, `record?: {from,to}` | `results: [{sandbox, ticked, time, idle?, palette/flags/frames?}]` |
| `status` | `sandbox` | `time`, `block_ticks`, `fluid_ticks`, `block_events`, `moving_pistons`, `entities`, `idle` |
| `entities` | `sandbox` | `entities: [{type, pos, uuid, motion, yaw, pitch, design, still, collides, on_ground, box, state}]` |
| `summon` | `sandbox`, `entities: [{type, pos: [x,y,z] (decimals), nbt?}]` | `uuids` (a design's entities: deterministic UUIDs and entity randoms, tagged `mctas_design`; mobs refused) |
| `update` | `sandbox`, `positions`, `kind: neighbors\|self\|shape` | `changed` |
| `exec` | `sandbox`, `command` | runs a vanilla command in the sandbox |
| `viewers` | | `count`, `names` (game clients connected; worker started with `--listen`) |
| `camera` | `sandbox`, `pos`, `yaw`, `pitch` | moves viewers there in spectator mode |
| `play` | `sandbox`, `ticks`, `tps` | steps at a wall-clock pace for viewers |
| `hold` | `ms` | serves viewers without ticking |

* `raw` placement writes exact states with no neighbor/shape updates and no `onPlace` (flags
  `UPDATE_CLIENTS|KNOWN_SHAPE|SUPPRESS_DROPS|SKIP_BLOCK_ENTITY_SIDEEFFECTS|SKIP_ON_PLACE`);
  `update` is `Block.UPDATE_ALL`, like a player.
* `idle` = no scheduled block/fluid ticks, no block events, no moving-piston block entities, no
  falling blocks, and every design entity (`summon`) at rest: `still` = the last tick did not move
  it, horizontal motion exactly 0, vertical motion 0 unless it stands on the ground (a living
  entity carries gravity after its move every tick). A sandbox with no design entity is idle on
  the block conditions alone.
* `step` takes `entity_watch: {from, to, walk_from, walk_to}` (designs with entities): after every
  tick it records `entity_blocked` (a colliding or pushable entity overlaps the walk box), the
  first `entity_escape` (an entity box outside the watch box) and the first `entity_foreign` (an
  entity without the design tag: a drop).
* `rotate` takes `entities: [{type, nbt?}]` too and returns their data turned like structure
  templates turn entities (`Entity.rotate` / `mirror`): the new `Rotation` (and a hanging entity's
  facing), every other key as given.
* An entity's `state` is its saved data minus position, motion, UUID and the volatile fields (fall
  distance, fire, air, hurt time...), keys sorted, plus `mctas_rail` (the rail a minecart is on); a
  minecart's yaw and flip are left out (they record its last direction of travel).
* `flags[i]` for palette entry `i`: `collides` (any collision shape) and `full` (full cube).
* Requests can be pipelined: write many, then read the responses.

## The judge and the verdict cache

`crate::verify` (`crates/piston-door/src/lib.rs`) places a design the way the judge does (raw
paste, wire shapes, one self update per block, settle: `video.rs::place` does the same for
videos), then runs the checks of [rules.md](rules.md) at every position, each in a cleared
sandbox. `door leaderboard`, `door frontier`, `door shapes`, `door catalog` and `door gallery` share a
verdict cache keyed by design content (blocks + ports, FNV-1a): `designs/piston-door-3x3/archive/verdicts-v5.tsv`
(verdict, worst open/close ticks, rotation result), and a shape cache, `archive/shapes-v1.tsv`
(the swept shape: every cell the door occupies at rest and during one open/close cycle, and the
ground check). The file names are bumped when the rules change, so old entries are dropped (v5:
the rotation check).

## Videos

`door video` runs the real 26.3 client (renderer "renderpearl"; falls back from OpenGL to Vulkan
on Mesa llvmpipe) on an Xvfb display. A worker started with `--listen <port>` accepts it as a
spectator; `play` steps the sandbox at a fixed ticks-per-second while serving the connection, and
ffmpeg records the display. The client is unmodified except for a launcher class that hides the
HUD. Each flip plays until the door has settled.

The camera is the spectator's pose (`camera` teleports every viewer). For a moving camera the
recording loop in `piston-door/src/video.rs` ticks one tick at a time (`play 1`) on a wall-clock
schedule and, between ticks, re-poses the camera about every 25 ms (`camera` + `hold`): an
absolute teleport snaps the client's position (no interpolation), so the motion is as smooth as
the teleport rate. Title cards and labels are drawn in Rust with the client jar's bitmap font
(`mctas::mcfont`) and overlaid by ffmpeg: the static ffmpeg build has no `drawtext` (ffmpeg 7
needs harfbuzz for it). Xvfb is stopped with SIGTERM so it removes its `/tmp/.X<n>-lock`; locks of
dead servers are reclaimed. The X displays are shared by everything on a box, so only the
coordinator records.

**One recording per design** (`clips.rs`): every door is recorded once, as a clean clip (still
door camera, no card, no label; 1280x720 with the build framed for a centered 4:3 crop, x264 crf
16) in the clips folder (`~/videos/clips/<design hash>.clip.mp4`, its first frame `.clip.png`, its
sidecar `.clip.txt`: the design hash, the recording settings, the ticks played after each flip, the
first cycle's open/close ticks). A clip is recorded again only when the design (blocks, ports,
entities) or the recording settings change (bump `LOOK` in `clips.rs` when the recording itself
changes), or with `--redo`. Every published product is cut from it with ffmpeg, without starting
Minecraft, and carries a `.stamp` (a hash of its clip's sidecar and its card and label lines) so it
is rebuilt exactly when one of them changed: a new volume relabels a door in seconds.

* **Archive videos** (`door video` / `door video-all`): the title card (a still drawn in Rust,
  fading into the clip) then the clip with the corner label (the size `80b` big, `volume WxHxD, N
  blocks placed` small, the name), H.264 yuv420p, uploaded to the GitHub release `videos` as
  `<stem>.mp4`; then the catalog (`archive/README.md`) links them. `--relabel` cuts every video
  that has a clip again and records nothing.
* **Tick videos** (`door tick-video`, `tickvideo.rs`): one open/close cycle tick by tick, filmed
  with a still camera. Captions say when the lever is flipped, count the ticks after the flip until
  nothing moves anymore (the state the judge checks), and mark the second flip and the settled
  closed state. A 3x3 map shows the doorway as the server sees it after each tick, and a strip
  under the picture shows the whole phase (doorway clear, door closed, or in between) with a
  cursor on the current tick.
* **GIFs** (`door gallery`, `showcase.rs`): each board's best cut from its clip (the centered 4:3
  crop; a still camera: an orbiting one makes GIFs ~10x larger) into a looping GIF with a small
  label (palette per clip, 1.5x speed), uploaded as `<stem>.gif`; a leader without a clip is
  recorded first, `--gif-only` records nothing. `door leaderboard` shows the GIF of each board's
  leader (or of a design tied with it) when the release has one. The gallery also draws the night's
  timeline chart (`night-timeline.png`: the record of each board over time, from the archive headers).
* **Montages** (`door montage`): a board's record progression (solid: `archive/records.tsv`; other
  boards: records in archive order) filmed by ONE camera framing every design at once (each moved so
  its door is at the origin), so the machine visibly shrinks around a door that stays put; intro and
  outro cards, a label per clip (blocks, delta, how it got there from the captions), a poster grid.
  Uploaded as `<board>-progression.mp4/.jpg`.

## World export (`door world`, `world.rs`)

One Minecraft world (a save folder for the real game) holding every working design of the
archive, in rows by board, smallest first, each with its block count written above it in red
concrete and its name on a floating label, so they can be tried by hand in the real client. The
world is built by the official server jar (as `door vanilla-check` runs it): an empty void world
(nothing under or around the machines, like the judge's world), every design pasted with
`/setblock ... strict` in its settled rest state (the judge's sequence: raw placement, wire
shapes, a self update of every block, settle), so nothing fires while it is built. Creative mode;
a glass platform at the world spawn, away from the machines.

## The official-server cross-checks

`door vanilla-check`, `vanilla-build`, `vanilla-gadget` and `vanilla-fuzz` run the unmodified
26.3 server jar (`crates/mctas/src/vanilla`): a void superflat world in a temp dir, RCON for
commands, time frozen (`/tick freeze`) and advanced with `/tick step 1`, a minimal offline-mode
Rust client (the bot, `vanilla/bot.rs`) that logs in and right-clicks blocks exactly like a player,
and `save-all flush` + region files parsed back (block states, scheduled ticks, moving pistons) to
compare with the simulator cell by cell. Every apparent divergence so far was a harness bug, never
the simulator.
