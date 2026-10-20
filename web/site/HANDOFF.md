# REDSTONEPEDIA: handoff (track `site`)

Live: https://vjeux.github.io/minecraft-tas/ (branch `gh-pages`, one orphan commit per deploy).
How it works and every command: [docs/site.md](../../docs/site.md). Code: `crates/site` (the
generator, `mctas site build|publish|serve|smoke|deploy`), `crates/site-model` (data model, query
engine, card markup; shared with the browser and unit-tested natively), `web/site` (the filtering
UI, Rust -> WebAssembly; its own cargo workspace).

## Rebuild and redeploy (from the repository root, on main)

```sh
cargo build --release && ./target/release/mctas site publish
```

`publish` = `site build` + `site smoke` (skipped with a note when no headless Chrome is found; a
failed check stops the deploy) + `site deploy` (force-pushes gh-pages, retries a rejected push).
~75 s. Check the live site afterwards: `mctas site smoke --url https://vjeux.github.io/minecraft-tas/`.

Prerequisites on a fresh box: `rustup target add wasm32-unknown-unknown`,
`cargo install wasm-bindgen-cli --version 0.2.129 --locked` (must match web/site/Cargo.toml; or
reuse the deployed UI: `git fetch origin gh-pages && mctas site publish --pkg-ref origin/gh-pages`),
`gh auth status` (deploy pushes with `gh auth token`). Optional: Chrome for Testing's
`chrome-headless-shell` unpacked in `~/tools/chrome-headless-shell-linux64/` (smoke), the Nu HTML
checker `vnu` (`vnu --errors-only --skip-non-html site/`: 0 errors on every page so far).

## State (2026-10-20 00:10Z)

- 4637 builds: 1690 doors (cached verdicts of main's judge; 611 `door fill` variants folded under
  the best filling of their mechanism), 2860 clocks (the catalog's grouping), 87 circuits (77 from
  the studies' archives + the 10 calibration designs).
- 29 extendable family cards, 1747 members generated at build time by each family's generator:
  304 judged in the game (extend.tsv), 1405 redsim-checked at the formula's period, 38 formula only.
- Pages: the index (filters, clock time search, sort incl. newest first, URL-fragment links), one
  page per build (live 3D viewer, properties, design download, similar builds), one per family
  card, the door boards (mirrors LEADERBOARD.md), the families index, all.html, about.
- webrender's viewer renders every thumbnail and build page (contract in docs/site.md); clock
  traces (designs/clocks/traces) are passed as `data-trace` for the viewer's replay.

## Next

- Circuits: per-study galleries by axis value (like the clock periods) once the studies grow; the
  catalog.tsv is redundant with the archives' verdicts.tsv (read directly).
- A "new since" badge (builds archived after the previous deploy).
- Watch the viewer bundle size (4.2 MB wasm, 1.3 MB gzip): webrender plans to shrink it.

## Running processes

None left running by this track (local previews with `mctas site serve` are stopped at the end).
