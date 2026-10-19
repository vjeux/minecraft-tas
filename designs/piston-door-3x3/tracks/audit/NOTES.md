# Track audit: what our rules, checks, generators and wiring tools silently exclude

Brief (coordinator, 2026-10-19 05:00Z): vjeux asked "what are we missing?" for the third time;
every time a community design showed a bound that silently excluded real solutions. Find the next
ones mechanically, before he has to: run every check on the community 7x9 (c79), test every known
door's mechanism against every generator's space, list what each wiring tool cannot represent,
list every hard-coded rule. Result page: [docs/audit.md](../../../../docs/audit.md); full generated
report: [AUDIT.md](AUDIT.md); extracted mechanisms in catalog format: [MECH.tsv](MECH.tsv).

## Tools built
* `mctas door audit` (crates/piston-door/src/audit.rs): sections checks / mech / wiring / rules;
  `--explain <design>` prints one door's block paths, every move with the piston that made it
  (push k behind / pull / drag), membership per bound setting and the bounds its moves break.
* `families::generate_within` (the generator restricted to an allow-list: membership of a known
  piece set in the generator's space without enumerating the space) and
  `families::explain_within` (which door block / move has no option, the 2-move budget, pieces no
  option uses, the ORDER rule).
* `tracks/audit/c79-closed-ff.txt`: c79 with its real lever on (4,5)'s front face (`door patch`).

## What was found and done
* c79's only failing check was the repeater-input rig (lever left off -> 'not open'): fixed for both
  repeater implementations (4c483b9): c79 is record-ELIGIBLE (x32, 15/15).
* Lever polarity was still hard-coded in minimize's lever moves, `door relever` (9b1074d), cosearch's
  screen, satwire and boxproof (all three fixed on main by their tracks after the report: satwire's
  c79 check went from 157/485 piston-tick mismatches to 0/482), and mech-wire (handed off).
* Periodic rest (main 96a62d6) mirrored into ideal, props-dyn, the local-search screen, the redsim
  screen and the tile judge (one commit each); cosearch's screen got it on main (pockets).
* The redsim judge rejected EVERY gravity door (sand was not a full cube in `cosearch::full_cube`):
  fixed (088f15e).
* cosearch's grid refused off-plane levers: added on explore/audit-cslever (f6de2a2); main took
  pockets' parallel implementation (3d3f5c27).
* Generators: c79 is outside families v3 / fullmech / mechspace default (retracted-only 1-move
  pullers), inside mechspace `--ext-pullers`; the solvers (macro and exact) solve its piece set.
  Over the archive: 65 flat doors are outside the generator under EVERY flag, for two bounds nobody
  had named: an extended-at-rest piston is never lifted / carried (Gen::providers `!ext`), and a
  piston that only acts away from its rest cell is never placed (reuse iterates placed pieces only).
* connect and topology accept c79 and all 68 flat archived doors in their own boxes (no exclusion);
  `families --check` prints "topology: no" only because it runs in the mechanism's own box.

## Method notes
* Mechanism extraction follows moving-block entities: a NEW entity has `progress_o == 0` (and the
  previous frame had no same entity with progress 0); non-source entities are moved blocks, source
  entities mark the acting piston (extending: at base + facing; retracting: at the base). A moved
  block is attributed to an acting piston on its line (push k behind, pull 2 ahead), else to one
  with the same direction (slime / honey drag). Blocks not where expected (0-ticks leave no
  entity) are re-found within 2 cells.
* Membership is by piece SET (pistons, families coordinates, mirror included), in the box the
  mechanism sweeps (door blocks, mechanism piston bases and heads over the cycle). The generator's
  nominal paths need not match the real ones (c79's TM moves 3 times; its pieces are generated for a
  2-move path).
* `explain_within` judges each block with every target piece present (an over-approximation), then
  checks the real core (built in order from nothing) to catch the ORDER rule.

## Dead ends / caveats
* `cargo test --doc` cannot run on this box (no rustdoc binary): the gate is the lib tests (14).
* Section 1's sheet-label row is the audit's own count, not `door sheet`'s output.
* The wiring palette checks are static (block states, facings, plane) plus the mechanics read from
  the design and its recorded cycle; QC-fired and BUD-fired pistons are not detected yet.

## Next ideas
* Detect QC- and BUD-fired pistons in the extraction (power query at each firing tick) and check
  each wiring tool's model of them on real doors.
* Feed MECH.tsv's excluded mechanisms (fast-clean, hA, s4ff) to mechspace after the two generator
  bounds are relaxed, and to satwire / boxproof as known-answer tests.
* Rerun `door audit` after every merge of a generator or wiring tool: a door that moves from IN to
  OUT is a regression nobody would notice otherwise.
