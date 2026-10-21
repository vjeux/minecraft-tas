# REDSTONEPEDIA: handoff (track `site`)

Live: https://vjeux.github.io/redstonepedia/ (branch `gh-pages`, one orphan commit per deploy).
How it works and every command: [docs/site.md](../../docs/site.md). Code: `crates/site` (the
generator, `mctas site build|publish|serve|smoke|deploy`), `crates/site-model` (data model, query
engine, card markup; shared with the browser and unit-tested natively), `web/site` (the filtering
UI, Rust -> WebAssembly; its own cargo workspace).

## Rebuild and redeploy (from the repository root, on main)

```sh
cargo build --release
./target/release/mctas site publish --wait
```

`publish` = `site build` + `site deploy` (force-pushes gh-pages, retries a rejected push). ~75 s.
No browser end-to-end gate any more (vjeux 2026-10-20 14:20Z: "I think we're overreacting to a
single bug we had. Please remove the e2e tests"): `mctas site smoke` (headless Chrome; `--engine
webkit` for WebKitGTK desktop + phone; `--page` films pages) stays an optional dev tool, run only
when someone asks for it (`mctas site chrome` installs the headless Chrome it needs).
Every page loads the bundles, the CSS and the data by content hash (`?v=`), so a hotfix reaches
phones at once.

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

## State (2026-10-20 16:50Z, explore/site after main fd51ca0d2; box devvm59220, devvm59165 is gone)

- Plain player language (vjeux 13:06Z): every build page leads with what it does in one sentence
  with its own numbers (crates/site/src/plain.rs), then how to try it (no "Why build it" line:
  vjeux 23:52Z, "super useless"; the smoke checks no page has one); kinds in
  everyday words ("One-shot timer that restarts on every press (retriggerable monostable)");
  headlines in words ("reach 3", "on for 32 ticks", "slows by 3"); our test, measurements,
  properties and the design file in one collapsed Details.
- No design ids in lists (vjeux 13:11Z): cards, variation tables, family member rows and related
  lists show properties only; the name is in the URL, the downloads and the Details.
- Deleted pages (vjeux 13:35Z): boards, families index, everything, about (generate::DELETED_PAGES,
  smoke-checked); no page links in the footer.
- "Works in any direction, anywhere" replaces "robust" (directional / locational when it fails).
- hidden.tsv: the slime-free extenders, pext's world-record bar (only each N's holder; `!`
  exception lines keep the N = 2 slime-free holders), the dominated pulse-limiter-psub-L7, the
  coordinator's TEMPORARY hides (3 TNT launchers being re-judged on a floor; R7's placement-only
  or refused circuits until clk-rt's in-game re-judge: pulse, tff, memory, square dividers), and
  the class rule `*/@vibration-sensitive` (clk-obs's vibration test: only vibration-proof sculk
  designs are published). Take a temporary line out as soon as its designs leave the archive or
  pass (a stale pattern hides the new designs).
- Clocks (16:45Z): the catalog's rule `Row::shown` (no uneven output, nothing ruled out of
  survival): 2544 archived clocks are not published until clockinfra's retap brings the even
  ones back; an output pulsing several times evenly per cycle shows that period.
- Opening lines say what a build needs (plain::needs: TNT height, loaded containers, water, open
  sky); a build archived under two kinds shows once (kind-twin + "also a ..."); shulker machines
  timed with other items are one card; every header has the index's tab counts.
- One card per mechanism (doors by category, box, times, seamless tier and properties; clock
  twins; census clocks unless the record of their period); the Circuits tab opens on the first
  design of every kind; search knows the players' words; family pages in words; every clock has
  its output timeline; "faster than the smallest" on bigger but faster designs.

- Cards and build pages lead with the properties (kind in small capitals, the main property and
  blocks as the headline, secondary metrics, chips, the name small); circuit kinds are base kinds
  in words (the number is a property); lists sort by the main number, then blocks.
- `crates/site/hidden.tsv`: circuits not published (the slime-free piston extenders);
  `mctas site dominated` lists the material-only dominated designs for the coordinator to check.
- Study 11 (pulse multipliers, dividers, monostables, timers): study page with variations and
  linked rows (studies 1 / 2 / 5), per-kind explanations, demos.

- 5644 builds: doors (only OUR OWN work: `ours` in archive/provenance.tsv and no seed-rule
  match; `door fill` variants folded under the best filling of their mechanism), clocks (the
  catalog's grouping), 408 circuits in 10 studies (the archives; vjeux keeps the basic
  single-component ones: "The interaction model is useful"; the shulker loaders, TNT dupers and
  BUD switches have a Variations table on their study page: the best design per VARIATIONS.md
  variation, from the metrics).
- Extendable families: clock and circuit cards, members generated at build time by
  each family's generator (judged in the game / simulated at exactly the formula's value / formula
  only).
- Shulker loaders and unloaders run live in the 3D view (clk-rt cb9ac3d79; all 16 since clk-misc
  2a2a3f8b9: the viewer's world is the judge's sandbox); the site runs each one at build time with
  the viewer's own session to give its box timings (circuits.rs `view_run`; a machine with two OUT
  chests is timed over 4 boxes a chest); whether a design is shown as built is the viewer's call.
- A Litematica schematic next to every design and member (`designs/<kind>/<stem>.litematic`;
  `mctas litematic`, [docs/litematic.md](../../docs/litematic.md)): "Download .litematic" on every
  build page and family member row.
- Header (vjeux): the title, Doors / Clocks / Circuits, one search box (text; a period on the
  Clocks tab; a value `L 12` / `N 26` / `latency 0` on the Circuits tab, expanding tunable groups
  and extendable families to the exact member); the footer only says what the site was built from.
- Build pages open with the live 3D view (vjeux: "the redstone before the explanation"); circuit
  pages then explain themselves (crates/site/src/explain.rs: what it does with the design's
  numbers, how to try it with webrender's IN/A/B buttons and demo, the judge's test as a timing
  diagram, metric meanings, glossary); tunable groups (catalog.tsv `group`) show as one card with
  a value picker on every member page; equivalent layouts collapse under their shown design; study
  pages (circuits/<study>/) and a circuits overview. No simulator attribution anywhere (vjeux).
- webrender's viewer renders every thumbnail and build page (contract in docs/site.md: data-src,
  data-trace, data-autoplay, data-stimulus / data-events (a BUD's target changes) / data-phases /
  data-input-mode for circuits; a viewer
  with class `mcview-live-src` reloads on a data-src change keeping the camera: the tunable-group
  picker and the family pages' member rows swap in place, web/site/src/swap.rs).
- Related lists walk the study's axis (the smallest design of each other value); the judge's
  diagram has a position track for piston extenders (payload 0..N) and flying machines.
- No calibration / selftest circuits on the site.

## Next

- A "new since" badge (builds archived after the previous deploy).
- Watch the viewer bundle size (4.2 MB wasm, 1.3 MB gzip): webrender plans to shrink it.
- provenance.tsv and the seed rule disagree on 257 pockets-63b-43-* doors (kept out; c70b is
  fixing its classifier).

## Running processes

None left running by this track (local previews with `mctas site serve` are stopped at the end).
