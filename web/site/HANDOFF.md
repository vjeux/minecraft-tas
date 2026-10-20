# REDSTONEPEDIA: handoff (track `site`)

Live: https://vjeux.github.io/minecraft-tas/ (branch `gh-pages`, one orphan commit per deploy).
How it works and every command: [docs/site.md](../../docs/site.md). Code: `crates/site` (the
generator, `mctas site build|publish|serve|smoke|deploy`), `crates/site-model` (data model, query
engine, card markup; shared with the browser and unit-tested natively), `web/site` (the filtering
UI, Rust -> WebAssembly; its own cargo workspace).

## Rebuild and redeploy (from the repository root, on main)

```sh
cargo build --release && ./target/release/mctas site publish --wait
```

`publish` = `site build` + `site smoke` (skipped with a note when no headless Chrome is found; a
failed check stops the deploy) + `site deploy` (force-pushes gh-pages, retries a rejected push).
~75 s. Check the live site afterwards: `mctas site smoke --url https://vjeux.github.io/minecraft-tas/`.

ONE deployer (the coordinator, after merge batches). `site deploy` guards against two accidents
seen on 2026-10-20: (1) a deploy from a tree that lacks the commit the live site was built from
(the gh-pages commit message records it) would take features away: it refuses (merge, rebuild,
or `--force`); (2) GitHub Pages builds a branch about 10 times an hour, later builds error out
(17 deploys in an hour by three agents): it refuses at 9 builds in the last hour, or waits with
`--wait`, then follows the Pages build and fails when it errors.

Prerequisites on a fresh box: `rustup target add wasm32-unknown-unknown`,
`cargo install wasm-bindgen-cli --version 0.2.129 --locked` (must match web/site/Cargo.toml; or
reuse the deployed UI: `git fetch origin gh-pages && mctas site publish --pkg-ref origin/gh-pages`),
`gh auth status` (deploy pushes with `gh auth token`). Optional: Chrome for Testing's
`chrome-headless-shell` unpacked in `~/tools/chrome-headless-shell-linux64/` (smoke), the Nu HTML
checker `vnu` (`vnu --errors-only --skip-non-html site/`: 0 errors on every page so far).

## State (2026-10-20 02:15Z, explore/site 4071bbc5f)

- 5148 builds: 914 doors (only OUR OWN work: `ours` in archive/provenance.tsv and no seed-rule
  match; `door fill` variants folded under the best filling of their mechanism), clocks (the
  catalog's grouping), 303 circuits (the studies' archives + the calibration designs; vjeux keeps
  the basic single-component ones: "The interaction model is useful").
- Extendable families: 32 clock cards + 13 circuit cards, 1997 members generated at build time by
  each family's generator (judged in the game / simulated at exactly the formula's value / formula
  only).
- A Litematica schematic next to every design and member (`designs/<kind>/<stem>.litematic`;
  `mctas litematic`, [docs/litematic.md](../../docs/litematic.md)): "Download .litematic" on every
  build page and family member row.
- Header (vjeux): the title, Doors / Clocks / Circuits, one search box (text; a period on the
  Clocks tab; a value `L 12` / `N 26` / `latency 0` on the Circuits tab, expanding tunable groups
  and extendable families to the exact member); everything else in the footer.
- Build pages open with the live 3D view (vjeux: "the redstone before the explanation"); circuit
  pages then explain themselves (crates/site/src/explain.rs: what it does with the design's
  numbers, how to try it with webrender's IN/A/B buttons and demo, the judge's test as a timing
  diagram, metric meanings, glossary); tunable groups (catalog.tsv `group`) show as one card with
  a value picker on every member page; equivalent layouts collapse under their shown design; study
  pages (circuits/<study>/) and a circuits overview. No simulator attribution anywhere (vjeux).
- webrender's viewer renders every thumbnail and build page (contract in docs/site.md: data-src,
  data-trace, data-autoplay, data-stimulus / data-phases / data-input-mode for circuits).

## Next

- A "new since" badge (builds archived after the previous deploy).
- Watch the viewer bundle size (4.2 MB wasm, 1.3 MB gzip): webrender plans to shrink it.
- provenance.tsv and the seed rule disagree on 257 pockets-63b-43-* doors (kept out; c70b is
  fixing its classifier).

## Running processes

None left running by this track (local previews with `mctas site serve` are stopped at the end).
