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
| `mctas site build --out site/ [--viewer <dir>]` | exports `site/data/site.json`, one page per build (`doors/<stem>.html`, `clocks/<stem>.html`) and per extendable family (`families/<card>.html`), the design files (`designs/...`), generates every family member with the family's own `extend generator` and checks it in redsim, builds the UI into `pkg/`, copies webrender's 3D viewer bundle into `viewer/` |
| `mctas site serve --dir site/ [--port 8000]` | local preview (static server on 127.0.0.1) |
| `mctas site smoke --dir site/` | headless Chrome (`--dump-dom`) loads the index, a time search, a facet link, a door page and a family page, and checks the DOM the WebAssembly UI built against the engine's own answers |
| `mctas site deploy --dir site/ [--enable-pages]` | one orphan commit of the folder, force-pushed to `gh-pages` (temporary index: the working tree and branch are untouched) |

Needs: `rustup target add wasm32-unknown-unknown` and the wasm-bindgen CLI of the version
`web/site/Cargo.toml` pins (`cargo install wasm-bindgen-cli --version 0.2.129 --locked`); the
build checks both. `--no-wasm` skips the UI, `--pkg <dir>` takes a prebuilt one. `smoke` uses
Chrome for Testing's `chrome-headless-shell` (`--chrome`, `$CHROME`, or unpacked in `~/tools`).
Validate the HTML with the Nu checker: `vnu --errors-only --skip-non-html site/`.

## Where the data comes from (no JVM)

- Doors: every `.txt` of `designs/piston-door-3x3/archive` with door and lever ports, read through
  the judge's caches next to it (`leaderboard::cached_entries`: verdicts, swept shapes, ground
  checks, volumes; `properties-v1.tsv` and `properties-dyn-v1.tsv` for seamless tiers, frame,
  hallway, eligibility), boards and leaders as `door catalog` computes them, captions from
  `captions.tsv` / `records.tsv`, videos from the `videos` release (`gh`). A design main's judge has
  not cached yet is left out (listed on stderr).
- Clocks: `designs/clocks/archive/verdicts-v1.tsv`, one build per type exactly as `clock catalog`
  groups them (variants folded under their host, archived family instances hidden with them by
  default), cards from `card_groups`, judged members from `extend.tsv`.
- Members of extendable families: every n of the verified range, generated at build time; status
  `judged` (in `extend.tsv`), `redsim` (the generated design simulated at exactly the formula's
  period) or `formula`. Disagreements are printed as notes.

## The 3D viewer contract (webrender, `explore/webrender`)

Any element with `data-mcview="thumb"` (still frame) or `data-mcview="full"` (live: orbit, play,
step, toggle the input) and `data-src="<relative url of a design .txt>"` (optional `data-design`
inline text, `data-kind`, `data-tick`, `data-yaw`, `data-pitch`) is rendered by the viewer bundle
(`viewer/mcview.js` + `viewer/mcview_bg.wasm`, loaded with `import init from "./viewer/mcview.js";
init();`): lazily when visible, one shared WebGL context, thumbnails cached by (src, size), new
elements picked up by a MutationObserver. It adds `mcview-ready` (the site's CSS then hides the
element's fallback card) or `mcview-error`. Without the bundle the pages keep the fallback cards.

## The UI

`index.html` is a shell; `pkg/redstonepedia.js` fetches `data/site.json` and draws: kind tabs,
the clock time search (`48`, `2.5 s`, `1 min 30 s`, `40-60 gt`, `2-3 s`, `< 1 min`:
`site_model::parse_time_query`), text search, the main facet (door category / clock family),
block kinds and properties as tri-state chips (must / must not / either), numeric ranges, sort,
and 60 cards per page (more on scroll). The query is the URL fragment (`#kind=clock&time=2-3%20s`),
so every search is a link; facet counts are computed with every other filter applied.
