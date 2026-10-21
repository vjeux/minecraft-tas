# webrender handoff

The in-browser viewer and simulator of redstonepedia: Rust compiled to WebAssembly (wgpu on WebGL2,
redsim inside), embedded by REDSTONEPEDIA (the site, `crates/site*`, `web/site`). Full reference:
[docs/webrender.md](../../docs/webrender.md) (attributes, controls, commands, internals). This page
is the short version for whoever picks the work up.

## Where things are

| What | Where |
|---|---|
| Core (platform independent): design parser, redsim session, models, atlas, camera, mesh, renderer, live player | `crates/mcview/src/` (`session.rs`, `player.rs`, `scene.rs`, `models.rs` the viewer's own models, `gamemodel.rs` the game's block geometry, `entitymodel.rs` the game's entity models, `mesh.rs`, `atlas.rs` + `atlas/art.rs` hand-drawn tiles + `atlas/vanilla.rs` game textures, `circuit.rs`, `replay.rs`, `render.rs`, `tools.rs`) |
| Browser glue: DOM scan, thumbnails, live viewer UI, overlays | `crates/mcview-web/src/` (`lib.rs`, `live.rs`, `overlay.rs`, `gpu.rs`) |
| Commands (`mctas web ...`): shot, check, bundle, demo, serve, compare, fit, layout, click, snap, strip, palette, reference, game-texture, luma, swap, lose | `crates/web-cli/src/lib.rs`, headless Chromium driver `cdp.rs` |
| The shipped bundle (committed; the site copies it) | `web/viewer/mcview.js`, `web/viewer/mcview_bg.wasm` |

## Build, test, ship

- `cargo build --release`, then `mctas web bundle -o web/viewer` (wasm32 build + wasm-bindgen).
- Gates before a push: `cargo test --release -p mcview`; `mctas door selftest --quick` (with
  `https_proxy=http://fwdproxy:8080 MCTAS_MAX_WORKERS=16`); after a sim change
  `mctas web check -q --wasm web/viewer/mcview_bg.wasm designs/clocks/archive/*.txt
  designs/clocks/calibration/*.txt designs/circuits/fly/archive/*.txt` (today: 4016 clocks + 85
  flying machines agree native and wasm, 0 parse-only, 0 failures) and over
  `designs/circuits/*/archive/*.txt` (325 circuits agree frame by frame with circuit sim under
  their judge's stimulus and events (BUD switches), the judge's consumer lamps included; the
  shulker designs and redsim-unmodeled ones parse-only); after a UI change
  `mctas web demo ... -o /tmp/demo`, `mctas web serve /tmp/demo --port 8765`, then
  `mctas web click http://127.0.0.1:8765/index.html` (also `--touch --window 390,844 --scale 2`,
  and `--hold` on circuits), `mctas web layout <url> --window 390,844 --scale 2` (needs
  `data-autoplay` viewers), `mctas web swap <url> --to <src>` (in-place swap keeps the camera's
  direction, re-fitted only when the new build does not fit),
  `mctas web lose <url>` (WebGL context loss); `mctas web snap <url> -o x.png --seconds 6` to
  look at a page (prints its console). EVERY bundle: `mctas web luma --url <page>` over a page of
  many designs (a demo page of every 20th door, all circuits, every 25th clock: ~550 viewers,
  1.5 min) and `mctas web luma <all archived designs>` natively: no mostly-black frame. NOTE: a
  demo page carries a COPY of the bundle (`web demo` copies web/viewer): re-run `web demo` after
  `web bundle`, and build with plain `cargo build --release` (`-p mctas` does not relink the
  binary). After a recorder change (the Record button): `mctas web record
  '<demo page>?mcview-debug' -o /tmp/rec --index <i>` on a clock, a door and a TNT duper
  (headless Chromium has no H.264 encoder: VP9 WebM), `ffprobe -count_frames` each (codec, size,
  duration, frames, `color_range=tv`) and a contact sheet (`ffmpeg -vf
  fps=1,scale=400:-2,tile=2x5`) looked at; WebKit once: put a `data/site.json` (`{"version": 1,
  "generated": "", "commit": "", "builds": []}`) in the demo dir, copy its index.html to
  `rec.html`, then `mctas site smoke --dir <demo dir> --engine webkit --page rec.html --click
  'css:[data-mcview-record]' --frames 10 --interval 9000` must print `record: saved ...` for
  desktop and phone (WebKitGTK encodes through GStreamer: `dnf install gstreamer1-plugins-good`
  for VP9; without it the console says the browser cannot encode).
- Push to `explore/webrender` and tell the coordinator: it merges and deploys (ONE deployer;
  never deploy, never ask site to redeploy).

## Decisions that hold

- Textures: the game's own, fetched at run time by the browser from misode/mcmeta's block atlas
  of 26.3 (`atlas::vanilla::ATLAS_URL` / `INDEX_URL`, Cache API); nothing Mojang-owned is
  committed or deployed (`/tmp/vanilla`, `mctas web reference`, is a LOCAL reference only).
  The hand-drawn tiles are the offline fallback. Native tools download the same atlas into
  `~/.cache/mctas/mcmeta/26.3-atlas` (`--procedural` opts out).
- Camera: `clock shot`'s exactly (the judge's swept box at 85% of the full 70-degree frame, then
  the 64% crop) for `web compare`'s pictures; the viewer's own views (live and thumbnails) take
  the scored default camera (`scene::best_camera`: docs/webrender.md, Camera; `web shot --live
  [--dpr 2]` renders a live viewer's). The floor is the pictures' floor: its top one block under the lowest cell
  (`clock shot` and `door video` fill grass at lo - 2), ending 56 blocks around a clock (110 for
  a door), each grass block turned like the game's (position-seeded variant of 4).
- Texture array layers must never be a multiple of 6: wgpu's WebGL2 backend then takes the
  array for a cube-map array and every block samples BLACK (f141e8417, 180 tiles; the atlas
  pads and asserts). `mctas web luma --url` catches that class; the native renderer does not.
- Circuits: the contract with site (data-stimulus / data-phases / data-input-mode / data-events,
  inputs driven like `Circuit::actions`, one bar button per input, auto-demo with data-autoplay,
  a BUD switch's target and footprint actions replayed from data-events). TNT designs run live
  with redsim's own TNT entities and exact explosions (clk-misc); shulker designs run live with
  the judge's chests (redsim's driver).
- vjeux (2026-10-20): the view shows ONLY what redsim simulates exactly; anything else is shown
  as built with `not simulated: <reason>`, never animated by viewer-side logic.
  `Session::drawn_matches_simulation` (run by `web check` on every tick) enforces it; the audit
  and what the client's own rendering may add are in docs/webrender.md#only-what-redsim-simulates.
- The viewer mirrors the judges, which keep changing on main: the clock judge's lamp in an empty
  output cell (clocks only), R7's input devices (`circuit::assign_drivers` ported as
  `mcview::circuit::inputs`), strength inputs (`data-levels`), consumer lamps. `web check`
  catches the drift (thousands of failures after a judge change): read the judge's commit, port
  the rule, never weaken the check. Its mismatch lines name both sides' input devices and every
  differing cell.
- Coordinator protocol (vjeux 14:23Z): before reporting a commit, merge the latest origin/main
  and run the FULL selftest, `web check --wasm` on every archived clock and circuit, and `web
  luma` (native and --url); the browser gates (click / hold / swap / lose / layout) stay cheap
  insurance. Main moves every ~20 min: merge, gate, push, report the main commit merged.
- No attribution text in the UI (vjeux): the badge only says what to do.
- House rules: Rust only (no scripts, no Python), edit with the editor tools, never touch ~/bin
  or dotfiles, never message humans, commit as Christopher Chedeau <vjeuxx@gmail.com>.

## Open

- `mctas web compare` on the 8-clock set: pixel diff 6.5, coarse 3.9 (8.2 before the game's
  block models, 7.7 with them, 7.3 with the game's back-face culling of translucent faces, 6.5
  with the game's fog; over 63 random clock pictures 8.3; the grass itself is texel-exact: bottom
  band diff ~3). What is left: per-batch floor edges, neighbouring clocks and black bands in
  some real pictures (the top band alone: 10-38; not modelable), texture noise. Entity clocks
  compare worse (10.5 on the cart loops) only because the picture's moment differs (the cart is
  elsewhere on its loop); the models match the pictures. `--floor-margin` measures the floor's
  reach (56 stays the best mean).
- Dropped non-block items are flat double-sided sprites (the game extrudes them 1 px thick).
- The "use your own Minecraft .jar" picker is dropped (the mcmeta textures replace it).
