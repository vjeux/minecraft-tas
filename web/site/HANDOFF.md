# REDSTONEPEDIA: handoff (track `site`)

Live: https://vjeux.github.io/minecraft-tas/ (branch `gh-pages`, one orphan commit per deploy).
How it works and every command: [docs/site.md](../../docs/site.md). Code: `crates/site` (the
generator, `mctas site build|serve|smoke|deploy`), `crates/site-model` (data model, query engine,
card markup; shared with the browser and unit-tested natively), `web/site` (the filtering UI,
Rust -> WebAssembly; its own cargo workspace).

## Rebuild and redeploy (from the repository root, on main)

```sh
cargo build --release
./target/release/mctas site build --out site [--viewer <webrender bundle dir>]
./target/release/mctas site smoke --dir site        # headless Chrome checks of the built site
./target/release/mctas site deploy --dir site       # force-pushes gh-pages
```

Prerequisites on a fresh box: `rustup target add wasm32-unknown-unknown`,
`cargo install wasm-bindgen-cli --version 0.2.129 --locked` (must match web/site/Cargo.toml),
`gh auth status` (deploy pushes with `gh auth token`). Optional: Chrome for Testing's
`chrome-headless-shell` in `~/tools/chrome-headless-shell-linux64/` (smoke), the Nu HTML checker
`vnu` (`vnu --errors-only --skip-non-html site/`: 0 errors on every page as of this writing).

## State

- 3562 builds: 1043 doors (cached verdicts of main's judge), 2511 clocks (the catalog's grouping:
  one per type, variants + archived family instances hidden by default), 8 circuits (the judge's
  calibration set; `designs/circuits/<study>/archive/` is picked up when `circuit archive` lands).
- 29 extendable family cards, 1747 members generated at build time by each family's generator:
  304 judged in the game (extend.tsv), 1405 redsim-checked at the formula's period, 38 formula only.
- Clock time search (`48 gt`, `2.5 s`, `1 min 30 s`, `40-60 gt`, `2-3 s`, `< 1 min`) expands
  families to the member that hits the period. Door boards page mirrors LEADERBOARD.md.
- Interim pictures until webrender's viewer: clock `img/<stem>.jpg` and family `img/<base>.n<k>.jpg`
  linked from raw.githubusercontent.com (not copied), door board GIFs; door pages embed their mp4.
- Viewer contract agreed with webrender (session 084de54f, branch explore/webrender): elements
  with `data-mcview="thumb"|"full"`, `data-src`, `data-kind`, optional `data-trace`
  (designs/clocks/traces/*.trace, 27+ real-game recordings from clk-misc); the viewer adds
  `mcview-ready` and the site's CSS hides the fallback (picture or text card).

## Next

- When webrender ships `web/viewer/` (mcview.js + mcview_bg.wasm): `mctas site build --viewer
  <checkout>/web/viewer`, smoke with WebGL (`--enable-unsafe-swiftshader` is already passed), deploy.
- Circuits: read the archive's verdicts once `mctas circuit archive` defines them (circuits.rs
  reads `verdicts-v1.tsv` with `file` + `summary` columns and `# CIRCUIT:` lines already).
- Per-kind pages for circuits (axis galleries like the clock periods) when there are enough.

## Running processes

None left running by this track (local previews: `mctas site serve` on 127.0.0.1, stopped).
