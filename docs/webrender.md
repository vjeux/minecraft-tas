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
  `data-trace` (a full viewer plays this real-game recording, `designs/clocks/traces/<stem>.trace`,
  for the designs redsim does not model: boats, minecarts, fluids), `data-autoplay` (a full viewer
  starts playing).
* The viewer appends a `<canvas>` filling the element (absolute, so give the element a size) and
  adds class `mcview-ready` once it shows (hide your fallback child with
  `.mcview-ready .fallback {display:none}`); on failure `mcview-error` and a `title` with the reason.
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
  resets the view, a tap on a lever or button uses it; the camera stays above the ground. Bar of
  44 px buttons: reset (r), play/pause (space), one game tick (right arrow), speed (cycles 1x to
  100x of 20 gt/s, then 1/4x, 1/2x), flip the lever (f, doors), tick counter in gt and seconds (and
  gt since the last flip); a badge says what runs (redsim, a recording, or shown as built).

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
* `mcview::models` / `mcview::atlas`: block models as cuboids with per-face tiles (pistons and
  heads, observers, dust by power, repeaters by delay and lock, comparators by mode, lamps, bulbs,
  sculk, hoppers, doors, trapdoors, rails, ...) and 16x16 textures drawn procedurally (no game asset
  is shipped: the site is public). Unknown blocks get a tinted cube.
* `mcview::mesh`: face culling against opaque neighbors (and translucent faces between blocks of
  the same kind: water, glass), the game's directional shading and corner ambient occlusion,
  moving blocks (`PistonMovingBlockEntity`) interpolated between ticks like the client's
  `PistonHeadRenderer`, translucent faces sorted back to front, a floor of grass blocks.
* `mcview::render`: wgpu with WebGL2 in the browser (and Vulkan natively): MSAA, the tiles as a
  mipmapped texture array (no bleeding between tiles), sky gradient and distance fog.

## Commands

| command | what |
|---|---|
| `mctas web shot <designs> -o <png or dir>` | offscreen PNG with the browser's renderer (Vulkan; lavapipe on a GPU-less box: `dnf install mesa-vulkan-drivers`) |
| `mctas web check <designs> [--wasm <mcview_bg.wasm>]` | parser and clock-period agreement with the repo's tools (and with the shipped wasm) |
| `mctas web compare <designs> -o <dir>` | real-game picture (`designs/clocks/img`) next to the viewer's frame of the same moment and camera, with a pixel difference |
| `mctas web strip <design> -o <png>` | a grid of frames over time (a door from its flip, a clock from its still moment), half ticks apart: the animation |
| `mctas web sheet <pngs> -o <png>` | textures scaled up in a grid, to compare them by eye |
| `mctas web bundle -o web/viewer` | the wasm bundle (needs `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version =0.2.129`) |
| `mctas web demo <designs> -o <dir> [--autoplay]` | a standalone demo page (live viewers + a thumbnail grid) |
| `mctas web serve <dir> --port 8000` | serve it on localhost |

A page can be checked without a desktop: `headless_shell --use-angle=swiftshader
--enable-unsafe-swiftshader --virtual-time-budget=60000 --screenshot=x.png <url>` (EPEL package
`chromium-headless`) runs the bundle with WebGL2 in software (`--window-size=390,844
--force-device-scale-factor=2` for a phone); add `?mcview-debug` to the page URL for timings in the
console (`--enable-logging=stderr`). Animation frames do not advance under a virtual time budget,
so a screenshot shows tick 0 of a playing viewer.
