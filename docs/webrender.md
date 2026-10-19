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
  text itself. Optional: `data-tick` (the still frame's tick), `data-yaw`, `data-pitch` (degrees).
* The viewer appends a `<canvas>` filling the element (absolute, so give the element a size) and
  adds class `mcview-ready` once it shows (hide your fallback child with
  `.mcview-ready .fallback {display:none}`); on failure `mcview-error` and a `title` with the reason.
* Thumbnails: drawn when the element comes near the viewport (IntersectionObserver, 300 px
  margin), through ONE shared WebGL2 context, copied into the element's canvas, cached by
  (source, size, tick, camera) so a re-added card paints at once. A click dispatches a bubbling
  `mcview-open` CustomEvent (detail = the src).
* Still frame: a door at rest (settled after placement: closed); a clock at the most active tick
  of one period after its warmup (most changing cells on, nothing moving), the way
  `mctas clock shot` picks the real-game pictures; the orange stained-glass marker next to the
  output (`port output`, the pictures' rule), lime outlines on levers / buttons / input ports.
* Full viewer: drag to orbit, right-drag or shift-drag to pan, wheel to zoom, double-click to
  reset the view, click a lever or button to use it; bar: reset, play/pause (space), one game tick
  (right arrow), speed (1/4x to 100x of 20 gt/s), flip lever (f), tick counter in gt and seconds
  (and gt since the last flip).

## How it works

* `mcview::design` parses the design format (no dependencies, so it compiles to wasm);
  `mctas web check` compares it with `mctas::design` on every file.
* `mcview::session` places a design like its judge (doors: raw paste, wire shapes, self updates,
  settle; clocks: the clock judge's placement and `port start` kick) and steps redsim one tick at a
  time; its period detector is the clock judge's (`clock::sim`), and `mctas web check` requires the
  same warmup and period on every archived clock (2361 agree; designs redsim cannot simulate are
  shown as built).
* `mcview::models` / `mcview::atlas`: block models as cuboids with per-face tiles (pistons and
  heads, observers, dust by power, repeaters by delay and lock, comparators by mode, lamps, bulbs,
  sculk, hoppers, doors, trapdoors, rails, ...) and 16x16 textures drawn procedurally (no game asset
  is shipped: the site is public). Unknown blocks get a tinted cube.
* `mcview::mesh`: face culling against opaque neighbors, the game's directional shading and corner
  ambient occlusion, moving blocks (`PistonMovingBlockEntity`) interpolated between ticks like the
  client's `PistonHeadRenderer`, translucent faces sorted back to front.
* `mcview::render`: wgpu with WebGL2 in the browser (and Vulkan natively): MSAA, mipmapped atlas,
  sky gradient and distance fog.

## Commands

| command | what |
|---|---|
| `mctas web shot <designs> -o <png or dir>` | offscreen PNG with the browser's renderer (Vulkan; lavapipe on a GPU-less box: `dnf install mesa-vulkan-drivers`) |
| `mctas web check <designs>` | parser and clock-period agreement with the repo's tools |
| `mctas web bundle -o web/viewer` | the wasm bundle (needs `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version =0.2.129`) |
| `mctas web demo <designs> -o <dir>` | a standalone demo page (live viewers + a thumbnail grid) |
| `mctas web serve <dir> --port 8000` | serve it on localhost |

A page can be checked without a desktop: `headless_shell --use-angle=swiftshader
--enable-unsafe-swiftshader --virtual-time-budget=60000 --screenshot=x.png <url>` (EPEL package
`chromium-headless`) runs the bundle with WebGL2 in software.
