# CLOCKS — the clock study (vjeux 2026-10-19 14:54Z)

vjeux: "I want to do a study on clocks. Finding as many clock types for each tick count (or family of ticks).
Build the scaffolding for this and let agents find all sorts of clocks."

Same repo as the doors: github.com/vjeux/minecraft-tas (headless Minecraft with tick control + the exact
Rust simulator redsim). Read AGENTS.md first: its house rules apply unchanged.

## House rules (from vjeux, non-negotiable)
- RUST ONLY: every tool, search, driver and one-off is Rust in this workspace (a new `mctas` subcommand
  or a flag on one). No bash/Python scripts, no `sh -c` blobs with logic; the shell only types one command.
  When a tool is one flag short, add the flag. Edit files with the editor tools.
- Never message any human. Report to the coordinator session f1a42990-e9ee-4714-8593-b34b64ed175d.
- Commit identity: `Christopher Chedeau <vjeuxx@gmail.com>`. Never push untested code; a commit exists only
  when `git log -1` shows it. Work in your own clone `~/agents/<track>/minecraft-tas`
  (`git clone https://github.com/vjeux/minecraft-tas` is already done on every box at ~/minecraft-tas: clone
  from it locally, then `git remote set-url origin https://github.com/vjeux/minecraft-tas`), branch
  `explore/<track>`. Push:
  `git -c http.extraHeader="Authorization: Basic $(printf 'vjeux:%s' "$(gh auth token)" | base64 -w0)" push origin HEAD:refs/heads/explore/<track>`
  Gate every push on `cargo build --release` (Finished), no merge markers, `mctas door selftest --quick`.
- Never record videos (shared X displays): the coordinator records them.
- Shared boxes: `MCTAS_MAX_WORKERS` caps JVM workers; never kill processes you did not start.
- Never weaken a judge to make something pass.

## What counts as a clock (defaults set by the coordinator; vjeux may amend)
- A structure that, after being placed (and optionally ONE start kick at a declared `start` cell), keeps
  producing a periodic signal forever with NO further input. Self-starting and kicked clocks both count
  (flagged).
- PERIOD P (game ticks, gt) = the smallest P such that the whole structure's state (every block state, and
  pending scheduled ticks where the simulator exposes them) repeats exactly every P gt after a warmup.
  Also reported: the OUTPUT waveform per period at the declared `output` cell (or every changing
  component): on-ticks per period (duty), rising edges per period, pulse widths.
- Must work in the REAL GAME (`mctas clock verify`, the JVM): periodic for a long horizon at several
  positions and all 4 rotations (position/rotation-dependent clocks are recorded but flagged), no items or
  entities created, nothing destroyed, nothing outside its box changed.
- FAMILY = the set of block types whose state changes during one period (the active set), detected
  automatically (e.g. `observer`, `observer+sticky_piston`, `comparator+redstone_wire`,
  `redstone_torch+repeater`, `sculk_sensor+note_block`, `hopper+comparator+sticky_piston`).
- TYPE = the structure up to translation and the 4 horizontal rotations (mirror images noted).
- Ranking inside a period: fewest blocks placed, then smallest bounding-box volume, then the waveform.
- The study's product: for every period P (1, 2, 3, ... and long ones), every family that reaches it,
  the distinct types found and the smallest of each; and which periods are IMPOSSIBLE for a family
  (with the reason or an exhaustive census as proof).

## Layout
- Designs: `designs/clocks/tracks/<track>/` (your work), `designs/clocks/archive/` (coordinator only:
  verified clocks + `captions.tsv`), generated `designs/clocks/CATALOG.md` (never hand-edited).
- Docs: `docs/clocks.md` (definition, judge, families, commands) — one home, link to it.
- Live board: Space memory namespace 2034923423883327, folder `minecraft-tas/clocks/` (README, coverage.md,
  roster in `minecraft-tas/board/roster.md`). Read:
  `meta agents.memory read --namespace=2034923423883327 --root --path=minecraft-tas/clocks/README.md`
  Write: `meta agents.memory write --namespace=2034923423883327 --root --path=<p> --content-file=file:///tmp/x.md`

## Tracks
- `clockinfra` — the scaffolding and the judge (owns crates/clock*, docs/clocks.md, redsim).
- `clk-obs` — block-update clocks: observers, pistons/sticky pistons (moving observers, head pops),
  copper bulbs, note blocks, lamps, trapdoors/doors, target blocks, rails.
- `clk-rt` — redstone-tick clocks: torches (burnout!), repeaters (delays, locking), comparators
  (compare/subtract, fading), dust; parametric families for every period and long periods.
- `clk-sculk` — vibration clocks: sculk / calibrated sensors (frequencies, wool occlusion, back-power
  deafness), amethyst, every vibration source.
- `clk-item` — item clocks: hoppers, droppers, dispensers, crafters, comparators reading containers,
  item-count-tuned periods.

## Reporting
- A clock worth keeping (new period for a family, smaller than the known smallest, a new type): push it and
  send ONE line to the coordinator: `<track>: clock P=<period>gt <family> <blocks> blocks, <path>, <what's new>`.
  The coordinator re-verifies on main, archives, catalogs, and records the videos.
- Tool commits (one per change, clear message): ping the coordinator to merge into main.
- No time budget: keep going. Before ending, write `designs/clocks/tracks/<track>/HANDOFF.md` and push.
