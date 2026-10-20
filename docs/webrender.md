# The browser viewer (webrender)

Every design renders and runs in the browser: redsim (the exact simulator) and a voxel renderer,
both Rust compiled to WebAssembly (`crates/mcview` + `crates/mcview-web`), no hand-written JS.
REDSTONEPEDIA (the site) embeds it on every page: thumbnails in the filter grids, a live viewer on
each build's page.

## Embedding (the contract with the site)

```html
<div data-mcview="thumb" data-src="designs/clocks/x.txt" style="width:240px;height:180px">fallback</div>
<div data-mcview="full" data-src="designs/doors/y.txt" style="width:100%;height:480px"></div>
<script type="module">import init from "./viewer/mcview.js"; init();</script>
```

* The bundle is `mcview.js` + `mcview_bg.wasm` (wasm-bindgen `--target web`; the wasm is found
  next to the js via `import.meta.url`); `mctas web bundle -o web/viewer` builds it, and a built
  copy is committed at `web/viewer/`.
* Every element with `data-mcview` (or class `mcview`) is a viewer, including elements added
  later (MutationObserver). Mode: `data-mcview` (or `data-mode`) = `thumb` (default) | `full`
  (`live`).
* `data-src` = URL of a design `.txt` (the repo format, relative to the page) or `data-design` = the
  text itself. Optional: `data-tick` (the still frame's tick), `data-yaw`, `data-pitch` (degrees),
  `data-trace` (a real-game recording, `designs/clocks/traces/<stem>.trace`, played by a full
  viewer only when redsim cannot run the design itself: UNMODELED at placement or during its
  4000 gt check; boats, minecarts, item entities, armor stands and fluids run live in redsim),
  `data-autoplay` (a full viewer starts playing).
* The viewer appends a `<canvas>` filling the element (absolute, so give the element a size) and
  adds class `mcview-ready` once it shows (hide your fallback child with
  `.mcview-ready .fallback {display:none}`); on failure `mcview-error` and a `title` with the reason.
* Swapping a design in place (the site's variant picker): change attributes on the existing
  element, `data-src` LAST; the viewer re-reads every `data-*` attribute at that moment and
  loads the new design without flashing its fallback: a live viewer keeps the person's camera
  when `data-keep-camera` is set (unless the new build would be out of view), its play state and
  speed, and rebuilds its input buttons, tags and overlays; a thumbnail is drawn again. Live
  viewers that support it carry the class `mcview-live-src` once ready (`mctas web swap` checks
  it). A lost WebGL context (iOS drops the oldest when a page holds many; a GPU reset) is made
  again when the browser restores the canvas (`mctas web lose`).
* Thumbnails: drawn when the element comes near the viewport (IntersectionObserver, 300 px
  margin), through ONE shared WebGL2 context, copied into the element's canvas, cached by
  (source, size, tick, camera) so a re-added card paints at once. A click dispatches a bubbling
  `mcview-open` CustomEvent (detail = the src). They render in tasks, not animation frames, so
  background tabs and headless browsers draw them too.
* Still frame: a door at rest (settled after placement: closed); a clock at the most active tick
  of one period after its warmup (most changing cells on, nothing moving), the way
  `mctas clock shot` picks the real-game pictures; the orange stained-glass marker next to the
  output (`port output`, else the clock judge's own output: the pictures' rule), glowing while the
  output is on; lime outlines on levers / buttons / input ports. The complete build shows from tick
  0: the `port start` kick (a block placed last, a button pressed) is applied before tick 0 is shown,
  in the clock judge's order.
* Full viewer (touch first): one finger or the mouse drags to orbit, two fingers pinch to zoom and
  drag to pan (right-drag / shift-drag with a mouse, wheel to zoom), a double tap / double click
  resets the view, a tap on a lever or button uses it at any time (paused: it flips and the
  simulation starts, so the build responds); the camera stays above the ground and starts on the
  side every lever and button can be seen from (a door's lever is often behind the build from the
  pictures' side). Bar of
  44 px buttons: reset (r), play/pause (space), one game tick (right arrow), speed (cycles 1x to
  100x of 20 gt/s, then 1/4x, 1/2x), flip the lever (f, doors); badges at the top: what runs
  (redsim, a recording, or shown as built) on the left, the tick counter in gt and seconds on the
  right (fixed width, tabular digits: nothing moves while it runs, `mctas web layout` checks it).
* Circuits (`circuit:` designs, the contract with the site): `data-stimulus` (the judge's own
  stimulus, `circuit::stimulus` syntax: `in 40+ 44-; ...` or `pulse L every T x K [from S]`,
  input names = the port names, `in` for a bare `port input:`), `data-phases` (`from-to label;
  ...`), `data-input-mode` = `pulse` | `toggle` (default from the kind: edges, limiters,
  extenders, T flip-flops, delays, RS latches and counters pulse; gates, D latches and piston
  extenders toggle). Inputs are driven like `Circuit::actions` (the cell written with updates,
  default redstone_block / air; `lever` used at every edge; `button` used at rising edges). One
  44 px bar button per input, labeled with its name uppercased (IN, A, B, S, R, D, E): pulse =
  on while held (a tap = a 2 gt pulse), toggle = a tap flips it (shown pressed while on); a tap on
  the input's cell, its device or its tag does the same, and a press held there past 250 ms
  holds the input on until released (like holding its button). A press that begins on an input
  never orbits or pans the camera; nothing moves the camera by itself except a flying machine
  while it plays (piston extenders included: their payload comes back). Nothing in a viewer
  selects text or opens a callout on a long press (user-select / touch-callout off, no context
  menu). Lime tags name the inputs over their cells, orange tags the
  outputs (OUT, or the port name); a strip under the counter shows each output's last 40 gt.
  `data-autoplay` + `data-stimulus` = the auto-demo: the judge's stimulus replayed from tick 0
  (the left badge shows `demo · <phase>`, as wide as its widest phase from the start so it never
  resizes), looped until the person's first input (any button, tap, play/pause/step/reset); then
  the inputs are theirs.
* `data-events="<frame> <op> <x> <y> <z> [<state>]; ..."` (design coordinates, frames like
  `data-stimulus`: after placement): the judge's actions at cells besides the inputs
  (`circuit::stimulus::Schedule::events`: a BUD switch's target and footprint probes), replayed
  in the auto-demo (with `data-autoplay`, with or without inputs): `place` (setBlock with
  updates, flags 3, like a player), `clear` (removed with updates), `shape` (changed with shape
  updates only, flags 2), `update` (`updateNeighborsAt` the cell, nothing changes). `web check`
  replays the BUD switches' events through this parser frame by frame against `circuit sim`.
  Cyan tags name a BUD switch's TARGET cells.
* TNT dupers run live (redsim primes TNT); the primed TNT is drawn like the game's
  `TntRenderer` (swelling over the last 10 ticks of its fuse, flashing 75% white every 5) and
  moved by the viewer (`mcview::tnt`, redsim records only when and where TNT primes): the game's
  `PrimedTnt.tick` (gravity, the game's collision, air drag, ground friction, slime bounce) and
  `PistonMovingBlockEntity.moveCollidedEntities` (moving blocks push it, a moving slime block
  launches it at one block per tick); its sideways direction (random in the game) comes from a
  hash of the cell and time; it vanishes at fuse 0 without exploding (the judge removes it).
* Shulker loaders and unloaders (`circuit: shulker-loader|shulker-unloader`) run live like their
  judge (redsim's `redstone::shulker::Chests`, the driver `mctas circuit shulker compare` checks
  tick for tick against the real game): the judge's chest on each `items`, `boxes`, `output` and
  `reject` port (tags ITEMS, BOXES, OUT) placed after the design, the settle (40 gt), then the
  box chests and a loader's sources kept full and the outputs emptied before every tick
  (endless), with unstackables (27 per box: a box every ~11 s) unless the design sorts (a
  `reject` port: its kind's own item). The world is the judge's sandbox for the design (since
  2026-10-20, clk-misc): its level random mirrored (`World::mirror_level_random`) and its drops
  seeded from the seed the judge builds it with (`redstone::shulker::judge_sandbox`: the watch
  box, the design's blocks and ports plus 3, gives the seed and the forced chunks), so a dropper
  choosing among stacks (dropper75) and a hopper choosing among minecarts (the rail unloaders)
  draw what the game draws, and every archived shulker design runs live. A start lever is
  the visitor's to flip (the judge flips it at its start); always-on designs start by
  themselves. A design whose run redsim cannot follow (it runs 1000 gt ahead on load) is shown
  as built, with a still shulker box in the box cell (in front of the dispenser): none of the
  archive today. Chests and shulker boxes are drawn
  with the game's `ChestModel` / `ShulkerModel` and entity textures. `web check` runs every
  shulker design against a reference run built from the repo's tools (the clock judge's
  placement, `circuit::Circuit`'s ports, the judge's sandbox from `circuit::run::watch_box`,
  which must equal the viewer's box), as placed and after its start lever: the same frames,
  native and wasm. A design shown as built never offers inputs or input outlines.
* A circuit's camera frames its whole build (no real-game picture to match: clocks keep the
  pictures' framing, which may cut a tall build).
* Containers: the contents of every dropper, dispenser and hopper (what redsim models) float
  above each one, item icons with counts, live every tick; a bar button (grid icon) hides them.

## How it works

* `mcview::design` parses the design format (no dependencies, so it compiles to wasm);
  `mctas web check` compares it with `mctas::design` on every file.
* `mcview::session` places a design like its judge (doors: raw paste, wire shapes, self updates,
  settle; clocks: the clock judge's placement and `port start` kick) and steps redsim one tick at a
  time; its period detector is the clock judge's (`clock::sim`), and `mctas web check` requires the
  same warmup and period on every archived clock (2369 agree; designs redsim cannot simulate are
  shown as built). `mctas web check --wasm web/viewer/mcview_bg.wasm` also runs the detector inside
  the shipped wasm (natively, with the wasmi interpreter) and requires the same numbers: the
  browser simulates exactly like redsim.
* `mcview::replay` plays a real-game recording (JSON Lines from `mctas clock misc trace-export`:
  changed cells and every entity per tick), once through the warmup, then one period looped, with
  entities interpolated between ticks.
* Entities (redsim's entity list: boats and chest boats, minecarts and hopper minecarts, item
  entities, armor stands; redsim's falling blocks) are summoned like the judge (after the blocks),
  simulated live and drawn interpolated between ticks; their clocks' period follows the real-game
  judge's rule for entities (block frames over 4000 gt, 3 periods:
  `clock::sim::entity_clock_period`), which `web check` compares too. Water heights follow the
  game (source 8/9, flowing (8 - level)/9, falling and bubble columns full, water under water
  full), with no faces between fluid cells.
* `mcview::entitymodel`: entities drawn like the game's client (read with `mctas src --client`):
  `BoatModel` (both paddles where `animatePaddle` holds an idle boat; the chest boat's chest),
  `MinecartModel` with its display block (a hopper minecart's hopper, 0.75 scale, display offset
  1), `ArmorStandModel` in the default pose (`ArmorStand.DEFAULT_*_POSE`, arms hidden, base plate
  square to the world), each through its renderer's transform (`AbstractBoatRenderer`,
  `AbstractMinecartRenderer.oldRender`: a cart on a rail follows the rail's line and slope,
  `ArmorStandRenderer`), the boxes as `ModelPart.Cube` builds them (UV layout, mirror), lit like
  `light.glsl` (two lights, 0.6 + 0.4 ambient); dropped items bob and spin like
  `ItemEntityRenderer` (a block item: its block model at the `ground` scale 0.25; any other: its
  item sprite, double-sided); falling blocks: the block model half a block back
  (`FallingBlockRenderer`). The textures are misode/mcmeta's entity atlas (boats and chest boats
  of every wood, the minecart, the armor stand) and its whole item atlas, packed below the block
  atlas in the game texture when a page first draws an entity (`entities.png`,
  `entities.min.json`, `items.png`, `items.min.json` in a `data-mcview-textures` directory);
  until then (or without them) the viewer's stand-in boxes are drawn.
* A design that travels (`travel:` / `distance:`, the circuits' headers) runs in its judge's
  corridor (`circuit fly sim`'s box: the design, kick cell and `stop:` blocks swept `distance`
  cells, plus 3; a piston extender sweeps its N), never in its own box; `stop:` blocks are placed
  first; the floor goes under the whole path; the camera follows a flying machine while it plays; it loops where
  `circuit fly sim` ends the run (the corridor's edge, rest or a break). `mctas web check` runs
  every archived flying machine against `circuit fly sim`: same end tick and same frames, native
  and wasm.
* A circuit runs as its judge builds it: the judge's consumer lamp (`circuit::CONSUMER`) in
  every air output cell (`consumer_cells`: not for extenders and flying machines, nor where a
  piston may push a block in); the lamp shows the output (no orange marker there).
  `mctas web check` runs every archived circuit under its judge's stimulus against
  `circuit::run::sim_trace`: every cell's full state (comparator strength, container contents)
  at every frame agrees.
* `mcview::models` / `mcview::atlas`: block models as cuboids with per-face tiles (pistons and
  heads, observers, dust by power, repeaters by delay and lock, comparators by mode, lamps, bulbs,
  sculk, hoppers, doors, trapdoors, rails, ...). The tiles are the game's own textures, fetched
  at run time by the browser from misode/mcmeta's block atlas of the judge's version
  (`raw.githubusercontent.com/misode/mcmeta/26.3-atlas/blocks/atlas.png` + `data.min.json`, the
  mirror block viewers use; kept in the Cache API; `atlas::vanilla` maps each tile to its
  texture, tinting the gray grass, leaves and water): nothing Mojang-owned is in the repository
  or the site. Thumbnails wait up to 4 s for them; without them (offline, blocked, or
  `data-mcview-textures="none"` on the page's root element, which can also point to another
  directory holding `atlas.png` + `data.min.json` + `block_definition.min.json` +
  `model.min.json`) the hand-drawn tiles of `atlas::art` (colors
  measured from the game, shapes after its layout) and procedural ones are used. Native tools
  (`shot`, `compare`, `strip`) download the same atlas once into `~/.cache/mctas` (`--procedural`:
  the hand-drawn tiles).
* `mcview::gamemodel`: the game's own block GEOMETRY, from misode/mcmeta's summary of every
  block definition and model of 26.3 (fetched with the atlas, cached): variants by properties,
  multipart conditions (`OR`/`AND`, `a|b`), weighted variants picked from the block's position
  like the game (`Mth.getSeed` + Java `Random.nextInt`), model parents, `#texture` variables
  (26.3's `{sprite}` objects too), elements with their rotations (and rescale), faces with uv,
  rotation, cullface and tint (redstone power, plains grass and foliage), quarter turns and
  uvlock. Their faces sample the game's block atlas uploaded as one 2D texture beside the tile
  array (vertex layer 65535). Blocks without game geometry (block-entity renderers: chests,
  signs, banners, skulls...) and water keep the viewer's own models (`MCVIEW_NO_GAME_MODELS=1`
  natively: the viewer's models everywhere).
* `mcview::mesh`: face culling against opaque neighbors (and translucent faces between blocks of
  the same kind: water, glass), the game's directional shading and corner ambient occlusion,
  moving blocks (`PistonMovingBlockEntity`) interpolated between ticks like the client's
  `PistonHeadRenderer`, translucent faces sorted back to front, a floor of grass blocks.
* `mcview::render`: wgpu with WebGL2 in the browser (and Vulkan natively): MSAA, the tiles as a
  mipmapped texture array (no bleeding between tiles) beside the game texture, sky gradient, back
  faces culled in both passes like the game's chunk render types (a glass block shows its three
  near faces only), and the game's fog (`fog.glsl` + `FogRenderer.setupFog`): the overworld's
  environment fog (spherical distance, linear from 0 to 1024 blocks) and the render distance's
  (cylindrical distance `max(|xz|, |y|)`, from 6 chunks minus a tenth to 6 chunks; a zoomed-out
  camera pushes the render distance back so the build is never cut), toward the sky colour.

## Commands

| command | what |
|---|---|
| `mctas web shot <designs> -o <png or dir>` | offscreen PNG with the browser's renderer (Vulkan; lavapipe on a GPU-less box: `dnf install mesa-vulkan-drivers`) |
| `mctas web check <designs> [--wasm <mcview_bg.wasm>]` | parser and clock-period agreement with the repo's tools (and with the shipped wasm) |
| `mctas web compare <designs> -o <dir> [--zoom x,y,w,h] [--floor-margin m]` | real-game picture (`designs/clocks/img`) next to the viewer's frame of the same moment and camera (without the viewer's input outlines), with a pixel difference; `--zoom` writes a 4x crop of both, `--floor-margin` tries another floor reach |
| `mctas web strip <design> -o <png>` | a grid of frames over time (a door from its flip, a clock from its still moment), half ticks apart: the animation |
| `mctas web sheet <pngs> -o <png>` | textures scaled up in a grid, to compare them by eye |
| `mctas web bundle -o web/viewer` | the wasm bundle (needs `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version =0.2.129`) |
| `mctas web demo <designs> -o <dir> [--autoplay]` | a standalone demo page (live viewers + a thumbnail grid) |
| `mctas web serve <dir> --port 8000` | serve it on localhost |
| `mctas web layout <url> [--window 390,844 --scale 2]` | layout-shift check: the page's live viewers (give them `data-autoplay`) in headless Chromium for some seconds of real time; fails if any viewer, bar, counter or badge box moved |
| `mctas web click <url> [--touch --window 390,844 --scale 2]` | input check through real browser input events (DevTools protocol over a pipe): clicks (or taps) each live viewer's lever or button while paused and requires it to flip, the simulation to start and the build to react (a door opens, a lamp lights) |
| `mctas web snap <url> -o <png> [--seconds 6 --scroll 900 --eval <js>]` | a screenshot after some seconds of real time (playing viewers and demos advance), with the page's console lines (and a JavaScript value) |
| `mctas web click --hold <url> [--touch ...]` | presses and holds each live viewer's circuit input in the 3D view: on while held, off once released, the camera unchanged |
| `mctas web luma <designs...>` / `mctas web luma --url <page>` | mostly-black guard: every design's still rendered natively, or every viewer of a page measured in headless Chromium through the real bundle (the WebGL path: a texture the GL backend misreads only shows there); fails under a mean luma of 40 or over half near black |
| `mctas web reference -o <dir>` | LOCAL ONLY, never committed: the game's block textures from the client jar, to measure colors (`mctas web palette`) and compare (`mctas web tiles`, `mctas web sheet`) |
| `mctas web game-texture -o <png>` | LOCAL ONLY: the game texture the viewer samples (block atlas, item atlas and entity textures packed below it), with where each entity texture went |
| `mctas web entities <design> [--trace designs/clocks/traces/exact/<stem>.trace] [--from T --to T]` | the viewer's simulated entities (redsim: end-of-tick positions and yaw) tick by tick next to a real-game trace's, with the largest distance: all 55 exact traces 0.0000 over their whole length (a minecart on a rail is DRAWN at the rail's line, like `AbstractMinecartRenderer.oldRender`; that drawn pose is not what is compared) |

A page can be checked without a desktop: `headless_shell --use-angle=swiftshader
--enable-unsafe-swiftshader --virtual-time-budget=60000 --screenshot=x.png <url>` (EPEL package
`chromium-headless`) runs the bundle with WebGL2 in software (`--window-size=390,844
--force-device-scale-factor=2` for a phone); add `?mcview-debug` to the page URL for timings in the
console (`--enable-logging=stderr`). Animation frames do not advance under a virtual time budget,
so a screenshot shows tick 0 of a playing viewer.
