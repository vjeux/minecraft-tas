# REDSTONEPEDIA (the website)

REDSTONEPEDIA lists every build of this repository with its properties: the archived 3x3 piston
doors, the clocks (with their extendable families expanded member by member) and, as they land,
the circuit studies. Live: https://vjeux.github.io/minecraft-tas/ (the `gh-pages` branch).

Everything is Rust: `crates/site` (the generator, `mctas site ...`), `crates/site-model` (the data
model, the query engine and the card markup, shared by both sides and unit-tested natively) and
`web/site` (the filtering UI: Rust compiled to WebAssembly with wasm-bindgen; its own cargo
workspace so the main build never compiles it). The only JavaScript is what wasm-bindgen
generates and the one-line `import init ...; init();` each page uses to start a bundle.

## Commands

| command | what it does |
|---|---|
| `mctas site publish` | THE command after merging into main: `build` (all defaults) + `smoke` (when a headless Chrome is found; a failure stops the deploy) + `deploy` |
| `mctas site build --out site/` | exports `site/data/site.json`, one page per build (`doors/<stem>.html`, `clocks/<stem>.html`, `circuits/<study>/<stem>.html`) and per extendable family (`families/<card>.html`), the boards page, the design files (`designs/...`) with a Litematica schematic next to each (`<stem>.litematic`, [litematic.md](litematic.md); one that cannot be written fails the build), generates every family member with the family's own `extend generator` and checks it in redsim, builds the UI into `pkg/`, copies webrender's 3D viewer (`web/viewer/` of the checkout by default; `--viewer <dir>`, `--viewer-ref <git ref>`, `--no-viewer`) into `viewer/`; `--pkg-ref origin/gh-pages` reuses the deployed UI instead of building it, only when its `pkg/SOURCE` fingerprint matches the checkout's `web/site` + `crates/site-model` (else the build fails: rebuild with the wasm32 target and wasm-bindgen-cli 0.2.129) |
| `mctas site serve --dir site/ [--port 8000]` | local preview (static server on 127.0.0.1) |
| `mctas site smoke --dir site/ [--url <deployed site>]` | headless Chrome (`--dump-dom`) loads the index, a time search, a facet link, a door page and a family page and checks the DOM the WebAssembly UI built against the engine's own answers; two probe pages screenshot a door and a clock thumbnail through the 3D viewer (WebGL2 in SwiftShader) and count colors (a render has thousands, the fallback one); `--url` checks the live site. `mctas site publish` refuses to deploy without it (`--no-smoke` overrides) |
| `mctas site smoke --dir site/ --engine webkit` | WebKitGTK (the engine family of iOS Safari) through WebKitWebDriver + MiniBrowser in an Xvfb display of its own (CentOS: `dnf install webkit2gtk3 webkit2gtk3-devel xorg-x11-server-Xvfb`), desktop 1280x900 and phone 390x844 at device scale 2 with an iPhone user agent: the index's first thumbnail and the live views of a door, a clock, a circuit, a family page, two entity clocks, a TNT duper, a shulker loader and a BUD switch must become ready and draw a real picture (colors >= 40, mean luma >= 25, at most half black), the console clean, and the picker / family swaps keep the viewer. It caught the 2026-10-20 black-texture bundle (86-100% black on every view) |
| `mctas site smoke --dir site/ --engine webkit --page <path> [--page ...] [--frames 6 --interval 700]` | films pages in WebKit instead of checking: screenshots of each page every `--interval` ms, desktop and phone, into `$MCTAS_WEBKIT_SHOTS` (default /tmp/mctas-webkit-shots), with each frame's colors and brightness and the console |
| `mctas site chrome` | downloads Chrome for Testing's headless shell into ~/.cache/mctas (what smoke and publish use) |
| `mctas site deploy --dir site/ [--enable-pages]` | one orphan commit of the folder, force-pushed to `gh-pages` (temporary index: the working tree and branch are untouched; a rejected push is retried) |

Needs: `rustup target add wasm32-unknown-unknown` and the wasm-bindgen CLI of the version
`web/site/Cargo.toml` pins (`cargo install wasm-bindgen-cli --version 0.2.129 --locked`); the
build checks both. `--no-wasm` skips the UI, `--pkg <dir>` takes a prebuilt one. `smoke` uses
Chrome for Testing's `chrome-headless-shell` (`--chrome`, `$CHROME`, or unpacked in `~/tools`).
Validate the HTML with the Nu checker: `vnu --errors-only --skip-non-html site/`.

## Where the data comes from (no JVM)

- Doors: every `.txt` of `designs/piston-door-3x3/archive` with door and lever ports that is OUR
  OWN work (vjeux: "only things we generated, to avoid stealing people's work"): marked `ours` in
  `archive/provenance.tsv` (track c70b) and matching no seed rule (`community*`, `c79*`,
  `closed-b-fill*`, `satwire-c79*`, `satwire-63b*`, `pockets-63b*`, `c88*`, `door fill of` a
  community design: `site::doors::community_seed`); the others are never published (pages, data,
  design files, litematics, boards); a published door matching the seed rule fails the build, and
  `site smoke` checks it again. Read through
  the judge's caches next to it (`leaderboard::cached_entries`: verdicts, swept shapes, ground
  checks, volumes; `properties-v1.tsv` and `properties-dyn-v1.tsv` for seamless tiers, frame,
  hallway, eligibility), boards and leaders as `door catalog` computes them (over the published
  doors), captions from
  `captions.tsv` / `records.tsv`, videos and board GIFs from the `videos` release (`gh`). A design
  main's judge has not cached yet is left out (listed on stderr). The `door fill` variants of one
  mechanism (`note: door fill of <design>`, with that design when archived) fold under the best
  of them (size, then blocks); the boards page still ranks them all, like LEADERBOARD.md.
- Clocks: `designs/clocks/archive/verdicts-v1.tsv`, one build per type exactly as `clock catalog`
  groups them (variants folded under their host, archived family instances hidden with them by
  default), cards from `card_groups`, judged members from `extend.tsv`, real-game traces
  (`designs/clocks/traces/<stem>.trace`) passed to the viewer as `data-trace`.
- Circuits: each study's `designs/circuits/<study>/archive/verdicts.tsv` rows and `captions.tsv`,
  variants folded like `circuit catalog` (same kind and axis value); the judge's calibration and
  selftest designs (`designs/circuits/calibration/`, `selftest/`) are not published (vjeux: "not
  useful to people"; smoke checks it). Every
  circuit is also run through `circuit::verify::sim` at build time: the judge's own stimulus
  becomes the viewer's demo (`data-stimulus`, `data-phases`, `data-input-mode`; a BUD switch's
  block changes at its target cells as `data-events`) and the page's
  timing diagram (inputs and outputs per gt, a TARGET row for the BUD judge's changes) with the
  checker's report; a design redsim cannot run is said to be shown as built, with the reason. The
  machines' studies (shulker loaders, TNT dupers, BUD switches) get a Variations table on their
  study page: the best design for each variation of their VARIATIONS.md, from the metrics
  (badges, items/s, rate, detected classes). The page's plain-language
  texts (what each kind does, how to try it, what each metric means, the studies' introductions,
  the glossary) live in `crates/site/src/explain.rs`. The catalog's folds (`catalog.tsv`):
  equivalent layouts (`variant_of` naming a design of the same kind, value, block count and block
  kinds) hide under their shown design ("Also N other layouts"), dominance variants as before, and
  a tunable group (`group`: one mechanism, other repeater delays) shows as one card (its smallest
  design, badge with the value span) whose every member page carries the group's value picker.
  The extendable circuit families (`extend` headers, `designs/circuits/families.tsv`) are cards
  like the clocks' (`families/<card>.html`, one per `extend group`; a card with several families
  shows its first base design in the grids): every n of the verified range generated with the
  family's own generator at build time (`designs/circuits/members/<family>-n<k>.txt` + its
  .litematic) and screened in redsim (`judged` from `extend.tsv`, `simulated` when the screen
  gives exactly the formula's value, else `formula`). The Circuits tab's search box takes a value
  (`L 12`, `N 26`, `12`, `10-20 gt`, `latency 0`: `site_model::parse_value_query`) and expands the
  groups and the families to the exact member. A build page starts with its live 3D view (vjeux:
  "Put the redstone before the explanation"), a group's picker under it, then the explanations.
- Interim pictures (the fallback under every 3D view): clock `img/<stem>.jpg` and family members'
  `img/<base>.n<k>.jpg`, linked from raw.githubusercontent.com (not copied), door board GIFs.
- Members of extendable families: every n of the verified range, generated at build time; status
  `judged` (in `extend.tsv`), `redsim` (the generated design simulated at exactly the formula's
  period) or `formula`. Disagreements are printed as notes.

## The 3D viewer contract (webrender: crates/mcview*, docs/webrender.md)

Any element with `data-mcview="thumb"` (still frame) or `data-mcview="full"` (live: orbit, play,
step, toggle the input) and `data-src="<relative url of a design .txt>"` (optional `data-design`
inline text, `data-kind`, `data-trace` = a real-game recording to replay, `data-tick`, `data-yaw`,
`data-pitch`; circuits: `data-stimulus`, `data-events` (the judge's block changes at a BUD's target:
`<frame> place|clear|shape|update <x> <y> <z> [<state>]; ...`, design coordinates), `data-phases`,
`data-input-mode`, `data-circuit`) is
rendered by the viewer bundle (`viewer/mcview.js` + `viewer/mcview_bg.wasm`, copied from the
checkout's `web/viewer/`): lazily when visible, one shared WebGL context for the thumbnails,
thumbnails cached by (src, size), new elements picked up by a MutationObserver. It adds
`mcview-ready` (the site's CSS then hides the element's fallback: the interim picture or a text
card) or `mcview-error`.

Cache-busting (GitHub Pages serves everything with max-age=600, and phones kept a broken viewer
after a hotfix): every page loads the bundles by content hash, `import init from
"./viewer/mcview.js?v=<hash>"; init({module_or_path: "./viewer/mcview_bg.wasm?v=<hash>"})` (the
wasm by its own URL: its default URL would drop the query, and a cached wasm must never meet a
new glue), the same for `pkg/redstonepedia*`, `style.css?v=` and `data/site.json?v=` (the index's
`#filters` carries it in `data-src`). The hashes are FNV-1a of the files, computed at build time
(`pages::set_versions`).

In-place swaps (vjeux: "Don't reload the page when clicking on variants, update inline and keep
the current camera"): a viewer that reloads its design when `data-src` changes marks itself with
the class `mcview-live-src` (webrender). On such a ready viewer, a build page's tunable-group
picker fetches the member's page and swaps everything around the viewer (`web/site/src/swap.rs`):
the viewer element stays in place, its `data-*` attributes are updated, then `data-keep-camera`,
then `data-src` last; `history.pushState` keeps the member's URL (popstate swaps back). A family
page has one full viewer (`#member-view`) whose design each member row (`a.view[data-src]`) swaps
the same way. Without `mcview-live-src` the links simply navigate.

## The UI

Every page's header is the title, the Doors / Clocks / Circuits tabs and one search box (vjeux:
nothing else at the top; about, door boards, families, circuit studies and everything sit in the
footer). `index.html` is a shell; `pkg/redstonepedia.js` fetches `data/site.json`, marks the
header's active tab with live counts and makes the header's box the live search: text, or on the
Clocks tab (or with a unit) the clock time search (`48`, `2.5 s`, `1 min 30 s`, `40-60 gt`,
`2-3 s`, `< 1 min`: `site_model::parse_time_query`); from any other page the box is a plain form
to `index.html?kind=<page kind>&q=<text>`, which the UI turns into the same query. The side panel
holds the main facet (door category / clock family / circuit kind),
block kinds and properties as tri-state chips (must / must not / either), numeric ranges, sort,
and 60 cards per page (more on scroll). The query is the URL fragment (`#kind=clock&time=2-3%20s`),
so every search is a link; facet counts are computed with every other filter applied.
