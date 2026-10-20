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
  `data-autoplay` (a full viewer starts playing). (`data-trace`, a real-game recording, is no
  longer played, vjeux 2026-10-20: the view shows only what redsim simulates exactly; a design
  redsim cannot run is shown as built with `not simulated: <reason>`.)
* The viewer appends a `<canvas>` filling the element (absolute, so give the element a size) and
  adds class `mcview-ready` once it shows (hide your fallback child with
  `.mcview-ready .fallback {display:none}`); on failure `mcview-error` and a `title` with the reason.
* Swapping a design in place (the site's variant picker): change attributes on the existing
  element, `data-src` LAST; the viewer re-reads every `data-*` attribute at that moment and
  loads the new design without flashing its fallback: a live viewer keeps the person's camera
  when `data-keep-camera` is set (its direction always; its distance and target too unless the
  new build does not fit it, then they are fitted again: never cut), its play state and
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
  `mctas clock shot` picks the real-game pictures. A clock's output (`port output`, else the
  clock judge's own output) is in the circuits' OUT box and tag (vjeux, 2026-10-20: not the
  pictures' orange stained-glass marker, which only `web compare` keeps); every output's box,
  clocks' and circuits', turns red (outline, a translucent red fill, its tag's text) while the
  output is powered: its own power state when it has one (a lamp lit, dust with power, a
  powered repeater or observer), else a redstone signal into its cell (a big dripleaf, a solid
  block); air never is (`Session::output_powered`, redsim's state every tick). A clock's empty
  output cell holds the clock judge's lamp (`clock::sim::judge_lamp`, clocks only: not a design
  with a `circuit:` kind or an input port), placed after the design's blocks; its OUT signal is
  the power delivered into the cell, not the lamp's lit state. Lime outlines on
  levers / buttons / input ports. The complete build shows from tick
  0: the `port start` kick (a block placed last, a button pressed) is applied before tick 0 is shown,
  in the clock judge's order.
* Full viewer (touch first): one finger or the mouse drags to orbit, two fingers pinch to zoom and
  drag to pan (right-drag / shift-drag with a mouse, wheel to zoom), a double tap / double click
  resets the view, a tap on a lever or button uses it at any time (paused: it flips and the
  simulation starts, so the build responds); the camera stays above a TNT duper's ground and starts on the
  side every lever, button and input cell can be tapped from at its centre (an empty input cell
  counts when a ray reaches it before any block, `scene::centre_point`), else from any point of
  it (a door's lever is often behind the build from the pictures' side; a gate's input behind its
  OUT lamp); no tag covers an input's cell. Bar of
  44 px buttons: reset (r), play/pause (space), one game tick (right arrow), speed (cycles 1x to
  100x of 20 gt/s, then 1/4x, 1/2x; a machine whose blocks repeat in under 10 gt, a 4 gt clock
  that would flicker at 5 Hz, starts at 1/4x, the button showing it, until the person picks a
  speed: `Session::short_cycle`; not a TNT design, whose flight and blast are the point), flip the lever (f, doors); a strip under the counter shows
  each output's last 40 gt (red: powered), captioned `output, last 2 s`, on clocks and circuits;
  badges at the top: what to do or what runs on the left, up to two lines on a narrow viewer and
  sized once from its longest text (`tap the lever`, `press the inputs`, the demo's phase,
  `not simulated: <reason>` for a design shown as built, `not simulated after tick N` when redsim
  meets something it does not model during the run: the view stops at the last exact state), the
  time in plain words on the right (`4.10 s · 82 ticks`; fixed width, tabular digits: nothing
  moves while it runs, `mctas web layout` checks it).
* Circuits (`circuit:` designs, the contract with the site): `data-stimulus` (the judge's own
  stimulus, `circuit::stimulus` syntax: `in 40+ 44-; ...` or `pulse L every T x K [from S]`,
  input names = the port names, `in` for a bare `port input:`), `data-phases` (`from-to label;
  ...`), `data-input-mode` = `pulse` | `toggle` (default from the kind: edges, limiters,
  extenders, T flip-flops, delays, RS latches and counters pulse; gates, D latches and piston
  extenders toggle). Inputs are driven like `Circuit::actions`: a plain input like a real build
  (the judge's R7, 2026-10-20: `circuit::assign_drivers` ported as `mcview::circuit::inputs`):
  the judge's delay-1 repeater in the first free side cell out of the build, a smooth stone under
  it, the judge's smooth stone in an empty input cell (or the design's own receiving block), and a
  redstone block one cell further out as its switch, placed or removed 2 gt before each input edge
  in the demo (the input cell's power changes at the stimulus tick) and at once on a tap (2 gt
  later through the repeater); an input with no way in keeps the old cell writing (the judge
  refuses the design); `lever` used at every edge; `button` used at rising edges. One
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
  (the left badge says the phase in words: `demo: A on, B off`, `demo: 2nd input pulse`,
  `demo: all inputs off`, `circuit::phase_words`; as wide as its widest phase from the start so it
  never resizes), looped until the person's first input (any button, tap, play/pause/step/reset);
  then the inputs are theirs.
* Doors with `data-autoplay`: the door demo flips the lever like the record videos (`door
  video`): 1.5 s closed, the lever on until the door settles open (`Session::idle`), 1.2 s open,
  the lever off until it settles closed, again; lever uses only (what runs is redsim's), the
  badge saying the stage (`demo: lever on, the door opens`, `demo: the door, open`, ...); the
  person's first flip, tap or bar input stops it.
* `data-events="<frame> <op> <x> <y> <z> [<state>]; ..."` (design coordinates, frames like
  `data-stimulus`: after placement): the judge's actions at cells besides the inputs
  (`circuit::stimulus::Schedule::events`: a BUD switch's target and footprint probes), replayed
  in the auto-demo (with `data-autoplay`, with or without inputs): `place` (setBlock with
  updates, flags 3, like a player), `clear` (removed with updates), `shape` (changed with shape
  updates only, flags 2), `update` (`updateNeighborsAt` the cell, nothing changes). `web check`
  replays the BUD switches' events through this parser frame by frame against `circuit sim`.
  Cyan tags name a BUD switch's TARGET cells.
* `data-levels="<frame> <input> <value>; ..."` (frames like `data-stimulus`, inputs by name,
  values 0..15): a strength input's values (`port input.a: x y z strength`, the judge's hex
  kinds, `circuit::stimulus::Schedule::levels`), replayed in the auto-demo like the judge: the
  input's chest placed again (its state unchanged), then its contents set to the value's fill
  (`circuit::strength_items`: the fewest cobblestone whose comparator signal is the value). A
  tap on a strength input flips it between 0 and 15. `web check` replays them against `circuit
  sim` (cal-hex-gt: 28201 frames). A circuit `circuit sim` cannot run (a random draw redsim does
  not model) must stop at the same tick in the viewer (`not simulated after tick N`).
* TNT designs (any TNT block) run live with redsim's own TNT and explosions (2026-10-20,
  clk-misc: `explosion::explode_on`, seed 0 with the world box's chunks, like the clock traces;
  entities tick only in the sandbox's forced chunks, so a TNT flung out of them freezes there and
  never explodes, as in the game):
  each primed TNT is redsim's `PrimedTnt` entity (its kick from the mirrored level random; moved
  by the engine: gravity, collisions, slime, moving blocks), drawn like the game's `TntRenderer`
  (swelling over the last 10 ticks of its fuse, flashing 75% white every 5), and at fuse 0 it
  EXPLODES exactly like the game (`redsim::explosion`): the blocks it breaks vanish, their drops fly
  as items, a TNT block it hits primes, other TNT is pushed; a blast destroying a moving block
  drops what it moves. A TNT duper with its explosions stands on its judge's ground (vjeux
  2026-10-20 17:53Z: "Please don't put a fake ground if the tnt goes through it"): real blocks of
  the simulated world laid like the judge's strict run (`circuit::run::sim_tnt`,
  `Circuit::ground_blocks`: grass over two dirt over bedrock, its top the archived safe drop
  under the design (`# CIRCUIT: ... safe drop N blocks`, 0 when absent), `reach` blocks around the
  design as written, in the judge's world box around it), placed raw once the design stands like
  `clock::sim::drive_tnt`. The reach is recomputed like the judge's: 24, doubled while a burning
  TNT gets more than `reach - 1` blocks sideways from the design within the viewer's first 600
  ticks (the judge's stimulus replayed for a triggered or switchable duper), at most 96 (cached
  per design: `Session::with_ground`). Its TNT land, bounce and explode on it and the craters
  show: the ground's cells that changed (`Floor::changed`, rescanned every tick) and their
  neighbours are drawn as blocks, the rest by its intact faces (`mesh::floor`: the grass tops,
  each layer's side at its edge, the bedrock's underside); the machine's blocks come from its own
  box (`Session::machine_box`). A TNT flying past the ground's edge falls into the void, as in
  the judge's world, or freezes where it leaves the forced chunks.
  `web check` runs each TNT design against a reference built from the repo's tools
  (`clock::sim::drive_custom` with the same configuration, the same ground): the same blocks and
  entities every tick for 400 ticks, native and wasm. The camera frames the TNT's flight in
  redsim's run within 8
  blocks of the build (a launcher's TNT leaves the view as it flies on).
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
  as built (with the judge's chests, nothing else added): none of the archive today. Chests and
  shulker boxes are drawn
  with the game's `ChestModel` / `ShulkerModel` and entity textures. `web check` runs every
  shulker design against a reference run built from the repo's tools (the clock judge's
  placement, `circuit::Circuit`'s ports, the judge's sandbox from `circuit::run::watch_box`,
  which must equal the viewer's box), as placed and after its start lever: the same frames,
  native and wasm. A design shown as built never offers inputs or input outlines.
* Camera: every view (live viewers and thumbnails) fits the build's whole moving extent
  (`scene::view_points`: the corners of every cell it occupies or sweeps: its blocks now and as
  placed, a door's opening, a clock's swept box over a period, a piston extender's travel, its
  ports, inputs and marker, its entities and where redsim's run takes them in 200 gt, within 8
  blocks) from the pictures' direction at their cropped field of view: 80% of the width, between
  the bar and the badges in a live viewer (`live_window`: 74 px clear of the bottom, 46 px of the
  top at the viewer's own height), centred in a thumbnail (`THUMB_WINDOW`, the still build
  only: no extender travel or entity look-ahead); a build smaller than 3 blocks across and 2
  high in every direction is framed as if it were that big, centred (a 2-block clock stands in a
  little sky around). A long thin build (its plan 3 times longer than wide) is seen at 45 degrees
  along its length at the pictures' elevation (`scene::view_dir`: a 3/4 view, the camera closer)
  instead of side-on. The browser draws like a
  client with a 24-chunk render distance (`Renderer::view_chunks`; `web compare` keeps the
  pictures' 6). No ground but real blocks (vjeux 17:53Z): the judges' worlds are void under a
  design (only a TNT duper's judge lays a ground, above), so the viewer draws no surface there:
  the sky goes on under the horizon and fades into a quiet void tone (`Renderer::void_below`),
  nothing an item, a cart or a falling block could pass through. On a TNT duper's real ground the
  grass tops are lit like the game's smooth lighting: each corner at the mean sky light of
  the four air cells around it in the layer between the grass and the build (a covered column
  gets its neighbours' light less one), through the game's brightness curve at the default
  brightness (`mesh::layer_sky_light`, `mesh::light_brightness`): the build's shadow. `web check`
  fails when the viewer's look draws a ground (`Look::ground`) or a ground cell differs from the
  world's (`Session::floor_drawn` in `drawn_matches_simulation`).
  `web compare` alone keeps the real-game pictures' camera and grass (`scene::picture_camera`, 56
  blocks).
* Containers: on a shulker loader or unloader, what tells the story is labelled by default: the
  judge's port chests (their contents inside their tag) and the shulker boxes being filled or
  emptied; any other container there (hoppers, droppers, dispensers, chests, carts, chest boats)
  shows its label when tapped (a tap again hides it). On every other design all containers are
  labelled (an item counter's droppers hold its count). The bar's contents button (grid icon,
  gold while all are labelled) labels them all or only the story's. A label is ONE compact
  pill, live every tick: per item kind its icon (the game's item sprite, else its block texture)
  and total count (`1,344`); shulker boxes first, as `12/27` slots filled for one box or `×3`
  with a fill bar for several; at most two kinds, then `+n`. A shulker design's same-kind port
  chests that stand together share one tag with their letters (ITEMS A–C over the three chests
  a-c, their contents summed; OUT A, OUT B: `circuit::port_groups`); named outputs read `OUT A`,
  `OUT B`. Tags and labels are laid
  out every frame so that none overlap each other, an outlined cell, the badges or the bar, and no
  leader line crosses a tag or label (`labels::Layout`): one that would collide goes higher or
  aside with a thin leader line back to its cell; a label with no free spot is left out
  (`mcview::labels`: what a label says and the layout; `overlay.rs` draws).
* Outlines (live viewers; thumbnails have none): the inputs lime (when the design runs: something
  to press), every other circuit port in its tag's colour: OUT orange, ITEMS gold, BOXES lilac,
  REJECT pink, TARGET cyan (shown as built too). A driven input (R7) gets one lime box around its
  cell, the judge's repeater and its switch (a red redstone block while the input is on), so the
  judge's equipment reads as part of the input it drives.
* Record (vjeux 2026-10-20: "add a record video button using mediabunny in the viewer"): the
  round red button over a live viewer's bottom right (above the bar; hidden where the browser
  has no WebCodecs `VideoEncoder`) records the viewer's own rendering of redsim's run from tick 0
  (blocks, entities, OUT and port boxes, tags and labels, the badges as shown): a separate player
  (the live camera, look, speed and demo) stepped exactly `1/30` s of game time per frame (exact
  partial ticks, never a screen capture of real time) for the whole run
  (`Player::recording_ticks`: the auto-demo's schedule; a door's demo cycle,
  `player::door_demo_cycle`; a clock's warmup and whole periods, at least one, about 8 s at its
  speed; a flying machine's flight; else 10 s; at most 20 s at the current speed). Each frame is
  drawn into the view's canvas and composed on a 2D canvas of the view's own pixels (at most
  1920x1080, both even): the 3D view, then the overlays painted from the page's own layout
  (`record::paint`: pills, text line by line, icons, the leaders and the output strip; the bar
  and the button are left out). The vendored mediabunny 1.62.0 (MPL-2.0,
  `crates/mcview-web/vendor/mediabunny/`, beside the bundle as `mediabunny.min.js` and
  `mediabunny.LICENSE.txt`, imported only when a recording starts: nothing at page load) encodes
  it: H.264 in MP4 (`fastStart: in-memory`) when the browser can encode it, else VP9 in WebM,
  4:2:0, 30 fps (`record::Encoder`), and the file downloads as `<design>.mp4|.webm`. While it runs
  the button shows the progress and a press stops it (no file); the live view comes back as it
  was. `mctas web record <url> -o <dir>` presses it in headless Chromium and keeps the file.

## Only what redsim simulates

vjeux (2026-10-20): the 3D view shows only what redsim simulates exactly; anything redsim cannot
simulate is shown as built with a plain `not simulated: <reason>`, never animated by
viewer-side logic. What the viewer draws is redsim's state of the tick; what it adds is the
game client's own rendering of that state: interpolation between the tick's start and end
positions (`partialTicks`), a minecart drawn on its rail's line (`AbstractMinecartRenderer
.oldRender`, from redsim's position), an item's bob and spin (`ItemEntityRenderer`, phase 0), a
primed TNT's swell and flash (`TntRenderer`), fluid heights from their levels; and annotations
(tags, labels, outlines, the OUT boxes red while their output is powered). Audit of 2026-10-20
(removed):

* the viewer's own primed TNT (`mcview::tnt`: a fixed kick, gravity, collisions, vanishing at
  fuse 0): deleted; primed TNT is redsim's `PrimedTnt` entity and explodes in redsim;
* falling blocks drawn from `y - vy` (a guess at last tick's height): drawn from redsim's
  position before the tick (kept by the session each tick);
* real-game recordings played for designs redsim could not run (`data-trace`): not played;
* a still shulker box added to shown-as-built shulker designs: not added;
* a run redsim reports an error in (`World::errors`: something it does not model) kept
  animating: it stops at the last exact state (`Session::halted`) and says so.

Kept (exact): the auto-demo replays the judge's own stimulus and events; the shulker designs'
chests are the judge's driver; the clocks' still frame and loop are ticks of redsim's run.
`Session::drawn_matches_simulation` checks it: every still block drawn is the world's block (a
quartz-like block simulated as stone drawn under its own name), nothing in an air cell, the
entities and falling blocks drawn are exactly redsim's at redsim's positions; `web check` runs it
on every tick of every archived design's run (circuits under the judge's stimulus, every frame;
clocks 120 gt; entity, TNT and shulker designs 400 gt) and fails on any difference.

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
* `mcview::replay` reads a real-game recording (JSON Lines from `mctas clock misc trace-export`:
  changed cells and every entity per tick) for the tools; the viewer no longer plays them (see
  [Only what redsim simulates](#only-what-redsim-simulates)).
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
  `PistonHeadRenderer`, translucent faces sorted back to front, a TNT duper's real ground
  (`mesh::floor`), the pictures' floor of grass blocks (`web compare`).
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
| `mctas web snap <url> -o <png> [--seconds 6 --scroll 900 --eval <js> --before <js> --before-at 4]` | a screenshot after some seconds of real time (playing viewers and demos advance), with the page's console lines (and a JavaScript value); `--before` runs JavaScript after `--before-at` seconds, then waits `--seconds` (an in-place swap: set `data-keep-camera` and `data-src`) |
| `mctas web click --hold <url> [--touch ...]` | presses and holds each live viewer's circuit input in the 3D view: on while held, off once released, the camera unchanged |
| `mctas web record <url> -o <dir> [--index 0 --window 1280,720]` | presses live viewer `index`'s Record button in headless Chromium and keeps the video it downloads (`<design>.mp4` or `.webm`) |
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
