# Working in minecraft-tas (for agents)

Agent workflow only. What counts as a door: [docs/rules.md](docs/rules.md). Every command:
[docs/tools.md](docs/tools.md). Mechanics read from the source: [docs/mechanics.md](docs/mechanics.md).
What was tried before (read it before starting a direction): [docs/history.md](docs/history.md) and
the track notebooks ([tracks/README.md](designs/piston-door-3x3/tracks/README.md)).

## House rules (from the repo owner, vjeux)
- **Rust only.** Every tool, harness, search, driver and one-off is Rust in this workspace
  (`crates/*`, usually a new subcommand of `mctas`). No bash or Python scripts, no `sh -c` blobs
  with logic: the shell only types one command. When a tool is one flag short of the job, add
  the flag. Edit files with the editor tools, not sed/perl.
- A new `mctas door` command goes in the door CLI crate of its area (`crates/door-*`: a variant
  in its `DoorCmd`, a line in its `run`, its `<Name>Args` and `cmd_<name>` in `src/cmd/<family>.rs`;
  no function over 150 lines); its work goes in piston-door, since the door crates are
  compiled unoptimized for fast builds ([internals.md#build](docs/internals.md#build)).
- **Never message anyone.** Report to whoever started you (your coordinator or vjeux).
- Never push untested code. A commit exists only when `git log -1` shows it.
- Commit identity: `Christopher Chedeau <vjeuxx@gmail.com>`.

## The judge, in short
- A door counts only if `mctas door verify <design> --extra-offsets 28` passes when built from
  **main** (3 open/close cycles at 32 positions); the coordinator re-verifies every record that
  way. The full rules, categories and boards are in [docs/rules.md](docs/rules.md).
- Never weaken or bypass the checks in `crates/piston-door/src/lib.rs` and never change how designs
  are placed (`piston-door/src/video.rs::place`). A design that only passes with a modified verifier
  does not count.
- Validate a mechanism with ideal inputs (`door drive`, `door ideal`) before wiring it; then wire,
  verify, shrink ([docs/tools.md](docs/tools.md#the-workflow-behind-the-records)). Proven circuits
  go in `designs/gadgets/` with a drive script.

## Shared boxes
- Several agents share each box. `MCTAS_MAX_WORKERS` (set in `~/.bashrc`) caps the worker JVMs
  of every pool; run at most two heavy processes at once. Never kill processes you did not start.
- Never record videos (the X displays are shared); the coordinator records records (`door video`,
  `video-all`, `gallery`, `montage`).

## Branches, files and pushing
- Work in your own clone (`~/agents/<track>/minecraft-tas`) on branch `explore/<track>`; never
  push to main or to another agent's branch. Push:
  `git -c http.extraHeader="Authorization: Basic $(printf 'vjeux:%s' "$(gh auth token)" | base64 -w0)" push origin HEAD:refs/heads/explore/<track>`
- Gate every push on the build (`cargo build --release` ends with `Finished`), on no merge
  markers left in `crates/` or `docs/`, and on `mctas door selftest --quick` (every line PASS,
  ~40 s, run from the repository root). A change to a search, judge, model or wiring tool also
  runs the full `mctas door selftest` (~2 min) before its push; a known answer that changes on
  purpose is updated in `crates/piston-door/src/selftest.rs` in the same commit, with the reason.
- Keep your track's files in `designs/piston-door-3x3/tracks/<track>/`: `BEST.txt` (your best
  verified design), `NOTES.md` (what you tried, what worked, what did not, and why), `archive/`.
  Add your track's line to `designs/piston-door-3x3/tracks/README.md`.
- Every time you beat your own best: `door verify <file> --extra-offsets 28 --archive`, copy it to
  `tracks/<track>/BEST.txt`, update NOTES.md, commit, push.
- Tool improvements that help everyone go in their own commits with clear messages; the
  coordinator merges them into main.
- Generated files are never edited by hand: `designs/piston-door-3x3/LEADERBOARD.md`
  (`door leaderboard`) and `designs/piston-door-3x3/archive/README.md` (`door catalog`). A new
  record or board leader gets a row in `archive/records.tsv` (solid records) or
  `archive/captions.tsv` (one line: how it works) instead.
- Docs have one home per kind of content (rules, mechanics, tools, internals, history: `docs/`);
  link to them instead of restating them.

## Reporting to the coordinator
- Every time you verify a design worth keeping (a best or an interesting variant, any board), push
  it to your branch and send ONE line to the coordinator session (its id is in your brief):
  `<track>: <blocks> blocks <category>[, slime-free][, open/close ticks], <path on your branch>, <what changed>`.
- End with a short report: best block count and category, the file path on your branch, the
  mechanism in a few lines, what failed and why, the next ideas worth trying. Push everything first.
