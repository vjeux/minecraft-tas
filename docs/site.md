# REDSTONEPEDIA (the website)

REDSTONEPEDIA lists ONE world-best design per kind of build (vjeux 2026-10-20 21:47Z: "I want to
have ONE world best implementation of each of these instead of a bunch of working but mid
implementations"): one 3x3 piston door card, one clock card, one card per circuit kind. Live:
https://vjeux.github.io/minecraft-tas/ (the `gh-pages` branch).

## One design per kind (`crates/site/src/kinds.rs`)

- **Circuits**: the kind is what a builder asks for (a gate's function with its mirror, a timer's
  restart rule, a sand elevator, the precise TNT duper; else the kind's first word). A
  non-parametric kind keeps one design: robust first, then fewest blocks, then the study's speed
  metric, the A-first version of a mirror pair; the gates with their own levers (the basics) are
  dropped. No design is picked by name (the coordinator 2026-10-21 06:56Z: new records must need
  no site change): where builders ask for more than the smallest, `RULES` narrows the kind first
  and the ranking picks among those (shulker loaders and unloaders: tiles side by side; the
  self-running TNT duper: works standing on the ground; the two-way flying machine: has an on/off
  lever), its card saying so ("the smallest of 16 tested that tiles side by side"); a rule nothing
  meets is a build note. `SIZE_PICKS` picks a size of a parametric kind (reach 2: the 1-wide flat
  rider). A parametric kind (reach, count, length, period, a lock's dials) keeps the best archived
  design of every size: one card (its smallest design, or the size builders know: `CARD_ROWS`,
  the 3-dial lock), and on each of their pages a table of every size.
- **Flying machines** (vjeux 2026-10-21 00:31Z): by engine shape, one design each, no size table:
  a flat machine (engine side by side, 1 block tall), a tall machine (engine stacked; its cargo
  capacity, from its `extend n` header, is a property) and a flying machine with docking stations
  (there and back); the other one-way machines (up, down, trains) are not published.
- **Doors**: one card, the best fully seamless door (the seamless board's record: vjeux 23:44Z,
  "This is what people usually consider to be piston doors"); every board's record keeps its page
  (no card) and every record's page has the table of records.
- **Clocks** (vjeux 2026-10-21 01:24Z: "show all of them with a card grouped by timing"): the
  Clocks tab shows every clock as a card, under one heading per period (ascending, smallest first
  within a period, nothing folded; each heading links its period and counts its clocks; the card
  leads with its blocks and parts), with a jump bar on top (landmark periods with their counts,
  every period in a list): every archived clock (exact twins folded) and every family member a
  judge verified (tested in the game, or simulated to exactly its period), which is a clock of its
  own with a page, a 3D view and downloads ("extended (n = k)"; its page links its tested base and
  says what each step adds, from the family's rule); a clock page's breadcrumb is its period. On
  the all-builds list the clocks come after the circuits. Each tab has its own opening line.
- **No extendable family pages** (vjeux 2026-10-21 00:33Z: "All these variations are useless,
  remove them"): no family pages, no member lists, no circuit members, no "comes in every size";
  the smoke checks no family page is written or linked and every member design file is an
  extended clock's.
- Everything else is not published at all (no card, page, design file or search hit); the files
  stay in the repository. `mctas site kinds` prints the plan (every kind, its pick or its rows,
  how many designs it leaves out).

Everything is Rust: `crates/site` (the generator, `mctas site ...`), `crates/site-model` (the data
model, the query engine and the card markup, shared by both sides and unit-tested natively) and
`web/site` (the filtering UI: Rust compiled to WebAssembly with wasm-bindgen; its own cargo
workspace so the main build never compiles it). The only JavaScript is what wasm-bindgen
generates and the one-line `import init ...; init();` each page uses to start a bundle.

## Commands

| command | what it does |
|---|---|
| `mctas site publish` | THE command after merging into main: `build` (all defaults) + `deploy`. No browser end-to-end gate (vjeux 2026-10-20 14:20Z: "Please remove the e2e tests"); the smoke below is an optional dev tool |
| `mctas site build --out site/` | exports `site/data/site.json`, one page per build (`doors/<stem>.html`, `clocks/<stem>.html`, `circuits/<study>/<stem>.html`) and per extendable family (`families/<card>.html`), the boards page, the design files (`designs/...`) with a Litematica schematic next to each (`<stem>.litematic`, [litematic.md](litematic.md); one that cannot be written fails the build), generates every family member with the family's own `extend generator` and checks it in redsim, builds the UI into `pkg/`, copies webrender's 3D viewer (`web/viewer/` of the checkout by default; `--viewer <dir>`, `--viewer-ref <git ref>`, `--no-viewer`) into `viewer/`; `--pkg-ref origin/gh-pages` reuses the deployed UI instead of building it, only when its `pkg/SOURCE` fingerprint matches the checkout's `web/site` + `crates/site-model` (else the build fails: rebuild with the wasm32 target and wasm-bindgen-cli 0.2.129) |
| `mctas site serve --dir site/ [--port 8000]` | local preview (static server on 127.0.0.1) |
| `mctas site smoke --dir site/ [--url <deployed site>]` | headless Chrome (`--dump-dom`) loads the index, a time search, a facet link, a door page and a family page and checks the DOM the WebAssembly UI built against the engine's own answers; two probe pages screenshot a door and a clock thumbnail through the 3D viewer (WebGL2 in SwiftShader) and count colors (a render has thousands, the fallback one); `--url` checks the live site. `mctas site publish` refuses to deploy without it (`--no-smoke` overrides) |
| `mctas site smoke --dir site/ --engine webkit` | WebKitGTK (the engine family of iOS Safari) through WebKitWebDriver + MiniBrowser in an Xvfb display of its own (CentOS: `dnf install webkit2gtk3 webkit2gtk3-devel xorg-x11-server-Xvfb`), desktop 1280x900 and phone 390x844 at device scale 2 with an iPhone user agent: the index's first thumbnail and the live views of a door, a clock, a circuit, a family page, two entity clocks, a TNT duper, five shulker machines (a loader, an unloader, the combo, the fastest loader and the fastest unloader) and a BUD switch must become ready and draw a real picture (colors >= 40, mean luma >= 25, at most half black), the console clean, and the picker / family swaps keep the viewer. It caught the 2026-10-20 black-texture bundle (86-100% black on every view) |
| `mctas site smoke --dir site/ --engine webkit --page <path> [--page ...] [--frames 6 --interval 700] [--click <label> ...]` | films pages in WebKit instead of checking: screenshots of each page every `--interval` ms, desktop and phone, into `$MCTAS_WEBKIT_SHOTS` (default /tmp/mctas-webkit-shots), with each frame's colors and brightness in the view, how much of the view changed since the previous and the first frame, and the console; `--click` presses viewer buttons by label first, in order (`IN` = a shulker machine's start lever; `1×` `2×` `5×` = speed 20×) |
| `mctas site chrome` | downloads Chrome for Testing's headless shell into ~/.cache/mctas (what the optional smoke uses) |
| `mctas site dominated [--all]` | the circuits a builder has no reason to pick: another design of the same kind and main value is smaller and at least as fast on the study's speed metrics, and the listed one's only advantage is a material restriction (it avoids slime, honey, torches, pistons or copper bulbs that the smaller one uses); a design with any other property the smaller one lacks (its own button, 1 wide, a badge, a different output, an extendable family) is not listed. Two lists: strictly slower, and the same speed. `--all` includes the designs `crates/site/hidden.tsv` already keeps off the site. `mctas site build` prints the same audit as notes at every build (`dominated (...)`: check, then add to hidden.tsv), so a re-archived study's new candidates show up without anyone remembering to run it. A family's base design (`extend` headers) counts as extendable, a builder's property: it is never listed |
| `mctas site deploy --dir site/ [--enable-pages]` | one orphan commit of the folder, force-pushed to `gh-pages` (temporary index: the working tree and branch are untouched; a rejected push is retried). The push carries a transport commit beside it (the same tree with the live site as parent, on `refs/mctas/gh-pages-transport`, never cloned) so git sends each changed file as a delta against its live version: a viewer-only change pushes 1.7 MB instead of 12.9 MB (every page's asset tags change); `--dry-run` prints both sizes. `--remote <path>` deploys to a local mirror (no Pages build to follow) |

Needs: `rustup target add wasm32-unknown-unknown` and the wasm-bindgen CLI of the version
`web/site/Cargo.toml` pins (`cargo install wasm-bindgen-cli --version 0.2.129 --locked`); the
build checks both. `--no-wasm` skips the UI, `--pkg <dir>` takes a prebuilt one. `smoke` uses
Chrome for Testing's `chrome-headless-shell` (`--chrome`, `$CHROME`, or unpacked in `~/tools`).
Validate the HTML with the Nu checker: `vnu --errors-only --skip-non-html site/`.

### Build time: the simulation cache (`crates/site/src/cache.rs`)

Every simulation a build needs (a clock member generated and simulated, the circuit judge's run
of a design, the 3D view's session on a circuit, a clock's chart and lamp check) is a pure
function of the design (`crates/site/src/simulate.rs`), cached by content under
`~/.cache/mctas/site/<domain>-<code>/` (`$MCTAS_SITE_CACHE=<dir>` elsewhere, `=off` none): the key
hashes the design's text, the parameters and the code that computes it (build.rs hashes each
domain's crates: members `mctas redsim clock clock-cli` + the generator's own crate, circuits
`mctas redsim clock circuit`, the view `redsim mcview`; every domain also hashes simulate.rs).
An unchanged design costs nothing; a change to a simulator recomputes what it touches; wording
edits elsewhere keep the cache (phase labels and sentences are written by the pages, not cached).
Failures are never stored. The build's last line says what was reused and computed; the 4 most
recently used code versions of each domain are kept. Every thumbnail's still frame (the viewer's
own search, `mcview::tools::still_attr`, webrender's `data-still`) is found here too, in the view
domain: 23 s cold for 4417 designs, nothing warm. The litematics carry their design's archive
time, not the build's: an unchanged site rebuilds byte for byte but for site.json's generation
time, so a deploy sends only what changed.

Measured on main 2026-10-21 (4414 builds, 1983 clock members): **warm 2.8 s** (80 MB), cold
**2 min 5 s** (was 4 min 28 s, 63 GB; cold peaks at ~75 GB). The cold pass is one member long:
`clk-item-dropring-1d-r1` n = 426 (the 5-minute instance, 866 blocks), whose generator's output
cell holds a comparator, so its output is searched again (`clock::sim::lamp_outputs`: a run per
candidate, ~50 s each); the others overlap it (members costliest first, the circuits beside the
clocks, every study's circuits in one pass).

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
  checker's report; whether the 3D view runs a design or shows it as built comes from the viewer's
  own session (`mcview::session::Session::new`, its `unsimulated` reason in plain words), and each
  live shulker machine is run at build time the way the view runs it (the judge's chests kept full
  and emptied, the start lever on) until it sends out 4 boxes: the page gives when the first box
  goes out and how often the next ones do, and the build prints every run (WARNING on none). The
  machines' studies (shulker loaders, TNT dupers, BUD switches) get a Variations table on their
  study page: the best design for each variation of their VARIATIONS.md, from the metrics
  (badges, items/s, rate, detected classes). The page's plain-language
  texts live in `crates/site/src/plain.rs` (vjeux 13:06Z: "explain all the builds so someone can
  understand them": every build page leads with one sentence of what it does with its own numbers,
  then why you would build it and how to try it; kinds in everyday words with the community's
  name in parentheses, `explain::kind_title` / `site_model::html::kind_words`) and
  `crates/site/src/explain.rs` (the in-depth texts, metric meanings, the studies' plain intros
  `STUDY_USES` and their "How we test" texts `STUDIES`). Our test, the measurements, the
  properties and the design's file name fold into one collapsed "Details" (vjeux 13:11Z: no design
  ids in any list or card; the name stays in the URL, the downloads and that details table).
  Captions that are agent or tool logs are dropped (`explain::caption_plain`); the first sentence
  of a designer's caption shows as "How this design does it". Clock pages show "Its output over
  time" (vjeux 13:46Z): the output over the start-up (shaded) and three periods, simulated from
  the design at build time the way the 3D view runs it (`pages::clock_timeline`; a clock whose
  design names no output shows its block that turns on most often, the census rule; a clock the
  browser's simulator cannot run says "Not simulated: <why>"). One card per mechanism
  (`generate::fold_twins`): doors with the same category, box, open/close times, seamless tier
  and properties fold under the smallest (listed on its page as its versions), clocks with the
  same period, blocks, parts and output carry the `twin` flag, and census clocks the `census`
  flag unless one is the record of its period; both are hidden from the default lists like the
  variants. Clocks follow the clock catalog's own rule (`clock::catalog::Row::shown`): a clock
  ruled out of a normal survival world (the verdicts' hidden column) or whose output pulses
  unevenly (clockinfra's `gaps` column; the coordinator's C38: a clock on the site delivers evenly
  spaced pulses at its output) is not published, and an output pulsing several times evenly per
  cycle shows that period as the clock's time (`ClockInfo::pulse_every`, `every()`; the cycle is
  its "full cycle"). Every build page says what the build needs in its opening lines
  (`plain::needs`: a TNT duper's height above the ground from its safe-drop metric, the items its
  containers start with from the design's `nbt:` lines, water, open sky) next to its warning
  (sculk, facing one way, one spot in the world). One build archived under two kinds (`same_build`)
  shows once on the Circuits tab (`kind-twin`, hidden unless its own kind is picked) with an
  "also a ..." chip on the other; shulker machines timed with other items (the same blocks) are
  one group ("works with any stack size"), and each shulker card leads with the variation it is
  the best design for (`Circuit::lead`). A bigger design strictly faster than every smaller one of its kind and value gets
  the `faster` flag ("faster than the smallest", `audit::mark_faster`). Circuits sort by
  `Circuit::rank` first (the first design of every kind, then the second...), then their study's
  place (`Circuit::order`; the gates with their own levers and the sand elevators are each
  study's basics, last). Search (`site_model::Site::run`): every build carries the community's
  words for its kind (`html::search_aliases`), builds whose kind the words name rank first, the
  players' names for extender reaches map to a value search (`with_synonyms`), and an empty
  search offers the nearest kinds (`html::nearest_searches`). Family pages lead with the kind
  and a title in words, how much each size adds, and keep the formula in a closed details.
  Pages vjeux deleted (13:35Z): the
  door boards, the families index, the list of everything and the about page
  (`generate::DELETED_PAGES`: never written, removed from an old output folder, and a smoke check
  fails if one exists or is linked). The catalog's folds (`catalog.tsv`):
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
`data-pitch`; circuits: `data-stimulus`, `data-levels` (a strength input's values over the judge's
test, `<frame> <input> <value 0..15>; ...`, the hex kinds), `data-events` (the judge's block changes at a BUD's target:
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
holds the main facet (door category / clock family / circuit kind: the base kind in words,
`Piston extender`, never one entry per N: the number is a property),
block kinds and properties as tri-state chips (must / must not / either), numeric ranges, sort
(by default the main number, then blocks: doors by size, clocks by period, circuits by kind, then
N / L / P...), and 60 cards per page (more on scroll). The query is the URL fragment (`#kind=clock&time=2-3%20s`),
so every search is a link; facet counts are computed with every other filter applied.

Every card (grids, study pages, related lists, family members, search results:
`site_model::html::props`) leads with the build's properties, not its generated name (vjeux:
"the focus is on the randomly generated name which is useless for the viewer"): the kind in small
capitals, the headline (a piston extender's `N = 3 · 19 blocks`, a clock's period and blocks, a
door's size and blocks, a divider's `÷3 · square`), a line of secondary metrics (extend / retract,
latency, re-arm, items/s...), chips (its own button or lever, 1 wide / 1 high, slime-free,
tunable span, badges) and the design's name last, small (and in the tooltip). A build page's
title is the same property line, the name under it. Circuits listed in `crates/site/hidden.tsv`
(study / file pattern, why) are not published at all (the slime-free piston extenders, vjeux:
"Why would anyone use this monstrosity?"); `mctas site smoke` checks none is.
