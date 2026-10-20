# webrender handoff

The in-browser viewer and simulator of minecraft-tas: Rust compiled to WebAssembly (wgpu on WebGL2,
redsim inside), embedded by REDSTONEPEDIA (the site, `crates/site*`, `web/site`). Full reference:
[docs/webrender.md](../../docs/webrender.md) (attributes, controls, commands, internals). This page
is the short version for whoever picks the work up.

## Where things are

| What | Where |
|---|---|
| Core (platform independent): design parser, redsim session, models, atlas, camera, mesh, renderer, live player | `crates/mcview/src/` (`session.rs`, `player.rs`, `scene.rs`, `models.rs`, `mesh.rs`, `atlas.rs` + `atlas/art.rs` hand-drawn tiles + `atlas/vanilla.rs` game textures, `circuit.rs`, `replay.rs`, `render.rs`, `tools.rs`) |
| Browser glue: DOM scan, thumbnails, live viewer UI, overlays | `crates/mcview-web/src/` (`lib.rs`, `live.rs`, `overlay.rs`, `gpu.rs`) |
| Commands (`mctas web ...`): shot, check, bundle, demo, serve, compare, fit, layout, click, snap, strip, palette, reference | `crates/web-cli/src/lib.rs`, headless Chromium driver `cdp.rs` |
| The shipped bundle (committed; the site copies it) | `web/viewer/mcview.js`, `web/viewer/mcview_bg.wasm` |

## Build, test, ship

- `cargo build --release`, then `mctas web bundle -o web/viewer` (wasm32 build + wasm-bindgen).
- Gates before a push: `cargo test --release -p mcview`; `mctas door selftest --quick` (with
  `https_proxy=http://fwdproxy:8080 MCTAS_MAX_WORKERS=16`); after a sim change
  `mctas web check -q --wasm web/viewer/mcview_bg.wasm designs/clocks/archive/*.txt
  designs/clocks/calibration/*.txt designs/circuits/fly/archive/*.txt` (today: 3494 clocks + 74
  flying machines agree native and wasm, 7 parse-only, 0 failures); after a UI change
  `mctas web demo ... -o /tmp/demo`, `mctas web serve /tmp/demo --port 8765`, then
  `mctas web click http://127.0.0.1:8765/index.html` (also `--touch --window 390,844 --scale 2`)
  and `mctas web layout <url> --window 390,844 --scale 2` (needs `data-autoplay` viewers);
  `mctas web snap <url> -o x.png --seconds 6` to look at a page.
- Push to `explore/webrender` and tell the coordinator: it merges and deploys (ONE deployer;
  never deploy, never ask site to redeploy).

## Decisions that hold

- Textures: the game's own, fetched at run time by the browser from misode/mcmeta's block atlas
  of 26.3 (`atlas::vanilla::ATLAS_URL` / `INDEX_URL`, Cache API); nothing Mojang-owned is
  committed or deployed (`/tmp/vanilla`, `mctas web reference`, is a LOCAL reference only).
  The hand-drawn tiles are the offline fallback. Native tools download the same atlas into
  `~/.cache/mctas/mcmeta/26.3-atlas` (`--procedural` opts out).
- Camera: `clock shot`'s exactly (the judge's swept box at 85% of the full 70-degree frame, then
  the 64% crop); the floor ends 48 blocks around the box (the pictures' sandbox floor edge).
- Circuits: the contract with site (data-stimulus / data-phases / data-input-mode, inputs driven
  like `Circuit::actions`, one bar button per input, auto-demo with data-autoplay).
- No attribution text in the UI (vjeux): the badge only says what to do.
- House rules: Rust only (no scripts, no Python), edit with the editor tools, never touch ~/bin
  or dotfiles, never message humans, commit as Christopher Chedeau <vjeuxx@gmail.com>.

## Open

- `mctas web compare` on the 8-clock set: pixel diff 12.6, coarse 6.5 (target < 10). What is
  left: per-batch floor edges and black bands in some real pictures (not modelable), texture
  noise, the leaves' brightness, the minecart / boat / armor stand models (block tiles; the
  entity textures are in misode/mcmeta's `entities/atlas.png`, not used yet), model geometry from
  the game's model JSON (piston head, repeater, comparator, hopper, dust).
- Water surfaces are flat per cell (the game slopes them by the neighbours' levels).
- The "use your own Minecraft .jar" picker is dropped (the mcmeta textures replace it).
