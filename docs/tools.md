# Tools

Every tool is a `mctas` subcommand, written in Rust (`./target/release/mctas ...` after
`cargo build --release`); `--help` on any command lists all of its flags. The rules the judge
enforces are in [rules.md](rules.md); how the simulator, the bridge and the videos work is in
[internals.md](internals.md).

## The workflow behind the records

1. Validate a mechanism with ideal inputs: redstone blocks placed and cleared by a `door drive`
   script, or a `door ideal` schedule. (`door mech-synth` finds mechanisms and writes both.)
2. Wire it, by hand or with `door route` / `door route2` / `door mech-wire`.
3. Run `door verify <design> --extra-offsets 28` (the judge, 32 positions).
4. Shrink it with `door minimize`, `door resynth`, `door grow` and `door regrow`.
5. Keep it: `door verify ... --archive` copies it into the archive with a header.

Proven circuits go in the gadget library, `designs/gadgets/` (a small design plus a drive script
whose `expect`s prove the behavior; catalog in [mechanics.md](mechanics.md#10-gadget-catalog)).
Run them all with `mctas door drive --quiet designs/gadgets/*.drive`.

## Setup and the game's source

| command | when to use it |
|---|---|
| `mctas setup` | once per machine: installs Java 25, the 26.3 server jar and libraries, and compiles the bridge (everything lives in `$MCTAS_HOME`, default `~/.cache/mctas`) |
| `mctas src decompile` | once: readable game source in `$MCTAS_HOME` (Vineflower) |
| `mctas src grep\|find\|show\|path` | to read the ground truth for a mechanic: `src grep 'class PistonBaseBlock'`, `src show PistonBaseBlock --grep getNeighborSignal -C 12` |
| `mctas sim repl` / `sim place` | talk to a simulator in JSON lines ([internals.md](internals.md#bridge-protocol)) / place a design and show what is pending |
| `mctas bench` | measure throughput and memory with many workers and sandboxes |

## The judge and looking inside a door

| command | when to use it |
|---|---|
| `door verify <design> [--extra-offsets 28] [--timeline] [--cycles N] [--archive] [--no-rotations] [--orientations] [--tile <spec>]` | the judge ([rules.md](rules.md)); prints blocks, category, open/close ticks and the rotation check. `--archive` keeps a copy of a working design in `archive/` next to it (skips duplicates); `--cycles 40` catches slow failures like oxidizing copper; `--orientations` runs all 8 orientations; `--tile wide:N\|period:P:N\|stack:N\|vperiod:Q:N\|step:dx,dy,dz:N` judges the design as a tile module ([rules.md](rules.md#tileable-doors-door-verify---tile); `--tile-out` writes the arrangement) |
| `door trace <design> [--close] [--ticks N]` | print every block change tick by tick after each lever flip: how a door actually moves |
| `door drive <script>...` | place a design, apply scripted inputs (`set`, `clear`, `use`, `step`, `quiet`, `print`, `plane`, probes, `expect`), print a plane per tick; any failed `expect` fails the command. Validates mechanisms with ideal inputs and runs the gadget tests |
| `door ideal <schedule> [--set name=value] [--sweep name=lo..hi]` | judge a mechanism driven by scheduled ideal inputs (`ideal.rs`) and sweep its timing windows: which input ticks work |
| `door reuse-check <design>` | verify many times in one reused sandbox: catches state leaking across runs (torch burnout memory) |
| `door frontier [--check <file>...] [--any-category] [--all]` | the Pareto frontier over (open, close, blocks) from the verdict cache; `--check` says whether a candidate lands on it or which doors dominate it |
| `door shapes [<design>...] [--branches] [--rule <preset or key<=n,...>]` | every working design's shape and swept shape relative to its door, the best rotation-safe design per shape rule (wall, wall2, flat, ground, no-above, narrow1, narrow2, wall-ground) and league, and the (volume, blocks) frontier |
| `door tile-scan <design>... [--tiles wide:3,period:4..12:3] [--levers one\|each]` | every design under every tiling, in parallel: which tilings work and the smallest working period |
| `door leaderboard [--fetch] [--sync-tracks] [--archive-dir DIR] [--cache-dir DIR] [--cached-only] --out designs/piston-door-3x3/LEADERBOARD.md` | judge every design on main and `explore/*` with main's verifier (verdicts cached by content in `archive/verdicts-v5.tsv`) and write the boards, each leader with its GIF, video and caption. `--archive-dir` also archives every working design; `--cached-only` simulates nothing |
| `door catalog [--videos DIR]` | write the archive catalog `archive/README.md` (every archived design: blocks, boards, ticks, rotations, buildability, video, how it works) from the verdict cache; records nothing |

## Building designs

| command | when to use it |
|---|---|
| `door gen <name> --out <file>` | print a built-in design (`designs*.rs`, `lm.rs`, `observer_seq.rs`) in the text format; `@name` also loads a built-in anywhere a design is expected |
| `door patch --out <file> <design> <script>` | apply an edit script: `set x y z <state>` (an extended piston gets its head), `clear x y z`, `port <name> x y z` |
| `door turn <design> --out <file> [--turns N] [--mirror]` | write a design turned and/or mirrored by the game's own block-state rotate/mirror; a quarter turn gives an east/west-facing door (`door-axis: z`): the way to build a variant for that direction |
| `door route <design> --start='x,y,z;..' --goal='x,y,z' [--allow=..] [--swept-design <file>]` | dust auto-router: fewest new blocks, avoids cells the machine sweeps, the walkway, the 3x3 columns in front of and behind the door, and touching other components; `--swept-design` takes the swept cells from the working design a net was cut from |
| `door route2 <design> --out <file>` | port-aware, timing-aware net router: rips up groups of up to `--max-rip` control blocks and re-routes every receiver to get the same signal tick for tick with fewer blocks (dust, repeaters, observers, conductors); `--need` routes a new net |
| `door place <design> --groups <file> --out <file>` | try every combination of alternative patch groups: wiring a mechanism whose part placements are a 3D puzzle |
| `door mech-synth <base> [--add k] [--pulses-only] [--verify N --out-dir DIR]` | mechanism synthesis in a macro piston model (a port of `PistonStructureResolver`): pistons, slime and blocks around the door, cheapest open and close paths, one symmetric event sequence for both lever edges; writes ideal-input designs and `door ideal` schedules and replays the best. Catalogs: `tracks/mech-synth/NOTES.md` |
| `door mech-wire <schedule>...` | wire a mechanism from its ideal-input schedule: route every piston cell's wave from a lever at each lever spot, rank by total blocks, judge the best (an upper bound: ~12 blocks worse than hand wiring) |

## Searching for fewer blocks or ticks

All candidates are screened at a few positions and accepted only by the full judge. The searches
(and the routers) keep the 3x3 columns in front of and behind the door clear. Every search takes
`--shape <preset or key<=n,...>` (only designs within those shape limits; the judge also checks the
swept shape) and `--tile <spec>` (judge each candidate as a tile module); `minimize`, `regrow`,
`grow` and `harvest` take `--rotation-safe` (accept only designs that also work turned).

| command | when to use it |
|---|---|
| `door minimize <design> --out <file> --moves all [--plateau] [--depth 2] [--archive]` | local search (`search.rs`): removals of 1-3 blocks, repeater delays, moves, substitutions, swaps (2-3 blocks out, 1 in), dust re-routing and net re-synthesis; `--plateau` walks neutral edits; `--objective open\|close\|total` optimizes speed; `--max-open/--max-close` keep a speed; `--to-sculk` lets it turn a solid design into a sculk one |
| `door harvest --out-dir DIR --moves all --plateau [--every 20]` | run that search on the best working designs of every branch and board not harvested yet |
| `door transplant` | learn edits from pairs of designs (a, then a smaller variant of a) and try each on the best designs of every board |
| `door grow <design> --out <file> --lo --hi --states [--k 2]` | add 1 or 2 blocks in a box to a design (usually one whose old wiring was cut), screening every combination; `--states` takes shorthands like `leaves` (persistent oak leaves) and `log` (3 axes); `--then-search --glue` seeds riders |
| `door regrow <design> --out <file> --size N` | cut every face-connected group of N control blocks and re-grow it with N-1 blocks; `--any` cuts any 2 blocks |
| `door synth <context> --lo --hi --from --to [--k K] [--palette ..] [--spec <script>]` | circuit synthesis by exhaustive simulation: every circuit of up to k blocks between two cell sets, judged by the door judge or a behavior script |
| `door resynth <design> --out <file> [--max-size 4] [--palette ..]` | large-neighborhood search by exact synthesis: clears every connected group of 2..max-size wiring blocks and tries every smaller circuit in its place (gravity 45 -> 42, flat 49 -> 43) |

## Correctness

| command | when to use it |
|---|---|
| `door buildable <design>... [--positions 32] [--lever off\|on] [--out-dir DIR]` | can a survival player build it? searches for a placement order (real clicks on visible faces, the world updating after each, temporary blocks allowed) that ends exactly in the judge's rest state, then runs the judge's cycles from there; writes `*.build.txt` scripts (`tracks/buildable/scripts/`, read by the catalog) |
| `door vanilla-check <design>... [--extra-offsets 28] [--trace]` | run designs in an UNMODIFIED vanilla 26.3 server (official jar, RCON, a bot player clicks the lever, region files read back) and compare every tick with the simulator under the judge's rules, plus one real-time cycle. PASS = identical tick for tick; DIVERGES names the first differing tick and cells. Method: `tracks/vanilla-check/NOTES.md` |
| `door vanilla-build <script.build.txt>... [--real-time] [--slow N]` | replay a `door buildable` script on the unmodified server with a bot player (item in hand, the script's look, sneaking, a right-click on the given face), compare after every action, then run the judge's cycles with the bot. Method: `tracks/vanilla-build/NOTES.md` |
| `door vanilla-gadget designs/gadgets/<g>.drive...` | replay gadget drive scripts on the unmodified server and compare every tick ([mechanics.md](mechanics.md#11-vanilla-confirmation)) |
| `door vanilla-fuzz [--count 50]` | differential fuzzing: random small contraptions compared between the simulator and the official server |

## Videos and showcase (coordinator only: the X displays are shared)

| command | when to use it |
|---|---|
| `door video <design> --out <f.mp4>` | record the door opening and closing with the real Minecraft client (title card, door camera, block-count label; each flip plays until the door has settled) |
| `door video-all --out DIR [--upload videos] [--dry-run]` | record every archived design that has no video yet, upload each to the `videos` release, then write the archive catalog; `--dry-run` lists what it would record |
| `door tick-video <design> --out <f.mp4>` | one open/close cycle tick by tick: still camera, captions for the flips and the settled states, a 3x3 map of the doorway per tick and a strip of the whole phase |
| `door card <design> --out <png>` | draw just a video's title card (and its corner label) |
| `door gallery --videos DIR [--upload videos]` | a looping GIF of the best design of every board, uploaded to the release (LEADERBOARD.md shows them), plus the night's timeline chart |
| `door montage --videos DIR --out <f.mp4> [--board solid] [--upload videos]` | a board's record progression as one video: every record filmed by the same camera, labeled with its block count and how it got there (solid: `archive/records.tsv`) |
| `door world --out DIR` | one Minecraft world (a save folder for the real game) with every working archived design in rows by board, to try them by hand |

## Generated files

Never edit these by hand; change their generator or its inputs and regenerate:

| file | written by | inputs |
|---|---|---|
| `designs/piston-door-3x3/LEADERBOARD.md` | `door leaderboard` | every design on main and `explore/*`, the verdict and shape caches, the `videos` release (GIFs, videos, montages), `archive/captions.tsv` |
| `designs/piston-door-3x3/archive/README.md` | `door catalog`, `door video-all` | the archive files, the verdict cache, `archive/records.tsv`, `archive/captions.tsv`, `tracks/buildable/scripts/`, the `videos` release |

`archive/records.tsv` (the solid record progression, one row per record) and
`archive/captions.tsv` (one line per design: how it works) are hand-edited data: add a row when
a design takes a record or tops a board.

## Design files

Contraptions are human-readable layers (`crates/mctas/src/design.rs`):

```text
# comment
name: tiny-demo
origin: 0 0 0                  # world position of row 0, column 0 (y adds to the layer y)
port lever: 2 1 0              # named positions (inputs, outputs, ...)
palette:
  S minecraft:stone
  P minecraft:sticky_piston[facing=up]
  L minecraft:lever[face=floor,facing=east]
layer 0:
  S.
layer 1:
  PL
```

Rows run north to south (z grows downward), columns west to east (x grows rightward). `.` and
spaces are air; every other character must be in the palette. A door design names the ports
`door` and `lever` ([rules.md](rules.md#the-door)); `@name` loads a built-in.
