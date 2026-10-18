# Working in minecraft-tas (for agents)

## House rules (from the repo owner, vjeux)
- **Rust only.** Every tool, harness, search, driver and one-off is Rust in this workspace
  (`crates/*`, usually a new subcommand of `mctas`). No bash or Python scripts, no `sh -c` blobs
  with logic: the shell only types one command. When a tool is one flag short of the job, add
  the flag. Edit files with the editor tools, not sed/perl.
- **Never message anyone.** Report to whoever started you (your coordinator or vjeux).
- Never push untested code. A commit exists only when `git log -1` shows it.
- Commit identity: `Christopher Chedeau <vjeuxx@gmail.com>`.

## The judge
- A door counts only if `mctas door verify <design> --extra-offsets 28` passes when built from
  **main** (3 open/close cycles at 32 positions). The coordinator re-verifies every record that way.
- Never weaken or bypass the checks in `crates/piston-door/src/lib.rs` (door, walkway, rest
  state, entities, legality, block count) and never change how designs are placed
  (`piston-door/src/video.rs::place`). A design that only passes with a modified verifier does
  not count.
- Rules in short (full text in `docs/piston-door-3x3.md`): ports `door` (min corner of the 3x3
  opening, plane `z = door.z`) and `lever` (an unpowered lever at rest). Closed: the 9 door cells
  hold 9 identical plain full blocks (no slime, honey, redstone parts, pistons...), the walkway
  (the opening extended 3 blocks in front and behind) is clear. Lever on: door cells + walkway
  walkable. Lever off: every block back exactly where it was, no entities. Score = non-air
  blocks, piston heads excluded. Survival-placeable blocks only (no command blocks, barriers,
  bedrock, headless pistons...). Any other block or mechanic in the game is allowed.
- Category: doors whose 9 blocks are gravity blocks (sand, gravel, concrete powder) are ranked
  separately (`category gravity` in the verify output), and so are doors using sculk sensors
  (`category sculk`: a player's footsteps or a mob nearby can trigger them); everything else is
  `category solid`, the main ranking.

## Tools (all `./target/release/mctas door ...`)
- `verify <design> [--extra-offsets N] [--timeline] [--archive]`: the judge. `--archive` keeps a
  copy of a working design in `archive/` next to it (skips duplicates).
- `drive <script>`: place a design, apply scripted inputs (`set`/`clear`/`use`/`step`/`quiet`/
  `print`/`plane`), print a plane per tick as ASCII. Validate mechanisms with ideal inputs
  (redstone blocks placed by hand) before wiring them.
- `route <design> --start='x,y,z;..' --goal='x,y,z' [--allow=..]`: dust auto-router (fewest new
  blocks; avoids cells the machine sweeps, the walkway, and touching other components).
- `minimize <design> --out <file> --moves all [--plateau] [--depth 2] [--archive]`: local search
  (search.rs): removals of 1-3 blocks, repeater delays, moves, substitutions, swaps (2-3 blocks
  out, 1 in), dust re-routing and net re-synthesis; candidates screened at 2 positions, accepted
  only by the full judge (32 positions). `--objective open|close|total` optimizes speed instead.
  Default `--moves singles` is the old greedy removal.
- `harvest --out-dir <dir> --moves all --plateau [--every 20]`: runs that search on the best
  working designs of every branch and league not harvested yet (harvest.rs).
- `gen <name> --out <file>`: canonicalize a design built in `crates/piston-door/src/designs.rs`.
- `mctas src grep|find|show`: read the decompiled Minecraft 26.3 source (the ground truth for
  mechanics; see `docs/mechanics.md`).
- Design files: layer text format (see any file in `designs/`); `@name` loads a built-in.

## Shared boxes
- Several agents share each box. `MCTAS_MAX_WORKERS` (set in `~/.bashrc`) caps the worker JVMs
  of every pool; run at most two heavy processes at once. Never kill processes you did not start.
- Never record videos (the X displays are shared); the coordinator records records.
- Work in your own clone (`~/agents/<track>/minecraft-tas`) on branch `explore/<track>`; never
  push to main or to another agent's branch. Push:
  `git -c http.extraHeader="Authorization: Basic $(printf 'vjeux:%s' "$(gh auth token)" | base64 -w0)" push origin HEAD:refs/heads/explore/<track>`
- Keep your track's files in `designs/piston-door-3x3/tracks/<track>/`: `BEST.txt` (your best
  verified design), `NOTES.md` (what you tried, what worked, what did not, and why), `archive/`.
