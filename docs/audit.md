# Audit: what our rules, checks, generators and wiring tools silently exclude

Every "none found" is bounded by a tool, and three times now vjeux showed us a community design
that one of our bounds had silently excluded (lever polarity, the open frame, extended-at-rest
pistons, a door block moving 3 times, droppers, normal pistons as signals). This page finds the
next ones mechanically, on doors that WORK: the community 7x9 (`tracks/c79/c79-closed.txt`, and
`tracks/audit/c79-closed-ff.txt` = the same door with its real lever on (4,5)'s front face), every
design in `archive/` (352) and the gadget library (87). Track notebook:
[tracks/audit/NOTES.md](../designs/piston-door-3x3/tracks/audit/NOTES.md).

```
mctas door audit [designs or folders...] [--sections checks,mech,wiring,rules] [--jvm] --out F [--tsv MECH.tsv]
mctas door audit --explain <design>      # one door: paths, every move and who made it, membership, bounds
```

The full generated report (every archived design, every section) is
[tracks/audit/AUDIT.md](../designs/piston-door-3x3/tracks/audit/AUDIT.md); the extracted flat
mechanisms with their macro runs, in families' catalog format (readable by `door connect`,
`door topology`, `boxproof catalog`), are in `tracks/audit/MECH.tsv`.

How section 2 works: one recorded redsim cycle; every door block and every piston is followed
through the moving-block entities (who pushed, pulled or dragged what, from where); the
MECHANISM is the pistons that move a door block or carry one that does (the others are signal
pistons); membership = families' OWN generator run restricted to the door's pistons
(`families::generate_within`, same code and order, only the pieces it may add are limited) in the
box the mechanism sweeps, under each bound setting; `families::explain_within` names the bound when
the answer is no.

## Fixed on explore/audit (each its own commit, build + lib tests gated)

| commit | what | what it admits |
|---|---|---|
| 4c483b9 | repeater input (props-dyn rig + `door properties`) for lever-ON doors: lever left off, repeater powered at rest, first input removes its block | c79 (both levers) record-ELIGIBLE: repeater ok x32 (15/15); lever-off designs unchanged |
| add7f90 | `door sheet` piston count excludes heads | label: c79 "14 pistons" -> 11 |
| 9b1074d | minimize's lever moves and `door relever` keep the design's lever polarity | every lever move / relever variant of a lever-ON door was broken |
| 9040a14, 5026010, 38f4b6f, a4c5a84, 610a8ab | periodic rest (main 96a62d6) in `door ideal`, props-dyn, the local-search screen, the redsim screen, the tile judge | doors / plans whose internal state alternates between cycles |
| 088f15e | redsim judge: sand / gravel / concrete powder are full cubes | ALL 33 archived gravity doors were rejected by `cosearch eval/search/agree`; the 15 without a lever in the tunnel now WORK at 32 positions |
| 2947ab6 + later | `door audit` itself, `families::generate_within` / `explain_within` (no change to the generator's output) | - |
| explore/audit-cslever f6de2a2 | cosearch search/pockets: a lever hung on a block's front/back face (on explore/cosearch2 + main) | c79 with its real lever as a seed: 5 finds in 20 s, JVM WORKS |

## 1. Checks on the community 7x9

c79 is a valid community door: everything vjeux did not forbid must pass. Before -> after this
track's fixes (and main's d975c15 / 96a62d6).

| check | c79-closed (lever on top of (4,5), flat file) | c79 with its real front-face lever |
|---|---|---|
| `door verify` (real judge, 4 positions) | WORKS 51 blocks, 14/14, rotation-safe, flat | WORKS; was "7x9x2", not flat -> flat since d975c15 (verify still PRINTS 7x9x2: the box counts the lever) |
| `cosearch eval` (redsim judge, 32 x 3) | WORKS, FULL, hallway ok, 63b (7x9x1) | same |
| `door properties` | FULL, volume 63 (7x9x1), lever on the surface; CLEAN: no ("door block 1 moves back": TM goes down then up, correct per 2.1.5.3) | same |
| `door properties-dyn` | hallway, contained, snow, steps, 20 cycles yes; repeater input NO -> yes 15/15 (4c483b9) | same |
| `door eligible` | not eligible (repeater) -> ELIGIBLE (judge x32 + repeater x32) | same |
| `door cycle` | CYCLES (6 flips x 4 positions, frame 12/12 open) | same |
| `door sheet --fill` | frame 12/12; "14 pistons" -> 11 (add7f90) | - |
| leaderboard boards | solid, slime-free, flat, 3-thick wall, seamless FULL + hallway; record-eligible only after 4c483b9; not "on the ground" (blocks below the door) | flat board only since d975c15 |
| minimize lever moves / `door relever` | placed unpowered levers (broke the door) -> keep polarity (9b1074d) | same |
| cosearch search screen | main: requires `Lever { powered: false }` (fixed on explore/cosearch2 c85570e) | main: "not flat" / grid refuses the lever -> explore/audit-cslever |
| `satwire --from-wired --check` | diverges at t0 on the lever chain, 157/485 piston-ticks (polarity; `--lever-on` auto-detected on explore/satwire 91730a2+) | same (+ `--lever-oop` needed) |
| boxproof bmc | no dropper cell, rest lever off: outside its space twice | same |
| `families --check` / `fullmech --check` (solvers) | both solve c79's piece set (families: open 11 + close 9, sym 12 with levels; fullmech: open 10 + close 9) | - |
| `door topology` (heuristic) | "topology: no" for the real door | - |
| `door connect` (region model) | c79's mechanism WIRABLE in its real 7x9 placement (x -2..4, y -3..5); NOT in 7x8 or 7x9 shifted up | - |
| c79-open-b (the screenshot, OPEN state) | judge "not closed at rest" (correct: the file rests open); `door sheet` draws flip 1 as open: "frame 3/12" (label; `door cycle` handles it) | - |

## 2. Mechanism generators

Columns: g1 families v3 (catalog: P <= 10, two 1, lifts, pushes; fullmech uses the same
generator), g2 mechspace gen default (P <= 11), g3 mechspace `--ext-pullers`, g6 every flag (P <= 16,
two 7, carry 2, vmid any, ext-pullers). Archive: 187 designs with a flat mechanism; IN under g1 /
g2: 95, under g3..g6: 97 (+ c79, c79-ff); 65 OUT under every flag (5 distinct mechanisms below +
2 gadget rows).

| door (files) | mechanism | g1 | g2 | g3 | g6 | the exact bound that excludes it |
|---|---|---|---|---|---|---|
| c79, c79-ff | 10 pistons: SE(-2,0..2) SW+(4,1) SW+(4,2) SE(-1,-1) SU(1,-2) SU(2,-2) SD(1,3) SD+(1,5) (+ signal ND(3,-2)); static frame (0,3) (2,3) (3,0); mechanism box 7x8 (the volume's 7x9 comes from the y=-3 wiring row) | out | out | IN | IN | 1-move exits use RETRACTED sticky pullers only (`Gen::others`, families.rs `if ext && !EXT_PULLERS`): MR/TR leave pulled by SW+(4,1)/(4,2) extended at rest. `--ext-pullers` exists only in mechspace; the catalog and fullmech never set it. Harmless path-level mismatches: BM is pushed from 3 behind through TM and C (providers: 1-2 behind), TM moves 3 times and back (paths: 1-2 moves, no reversal); the set is generated anyway via other nominal paths |
| fast-clean (21) + speed-wire (18): a1c, s4c, n3c, c1r, o2... | 11: SU+(1,-2) SE+(-2,0..2) SW+(4,0..2) SE(-1,3) SD(0,3) NW/SW(3,3) SD+(1,4) | out | out | out | out | a piston that only acts AWAY from its rest cell is never placed: SD(0,3) is pushed sideways to (1,3) by SE(-1,3) and only then pulls C; `Gen::providers` reuses (lifts / carries) only pieces an earlier provider already placed, and (0,3) is no provider position |
| hA-full-rl-dropper (14), eligible-s4ff (3), fresh-a-sf1-w2 (2) | 10: SU(1,-2) SE/SW(-2/4,0..2) SE+(-1,3) or SW(3,3), SD+(1,4) SD(2,4)/SD(1,5) | out | out | out | out | a piston EXTENDED at rest that is later lifted / carried is never reused: SD+(1,4) retracts, is pushed down to (1,3) by SD(1,5), then pulls C; lifts and `--carry` require `!ext` (`Gen::providers`) |
| gravity-sf1 (5) | 9 pistons, sand door blocks | out | out | out | out | door blocks FALL (BM (1,0)->(1,-2), C (1,1)->(1,-1)->(1,-3)): families' macro model has no gravity blocks |

Solvers vs generators: the macro solver (`families::solve`) and the exact solver
(`fullmech --check`) both solve c79's piece set; only the GENERATORS exclude it. Every OUT row above
is a generator bound, not a solver bound.

Wiring-feasibility models on known doors (the audit runs both on each door's own mechanism in the
door's own build box): `door connect` says WIRABLE for c79 and for all 66 flat archived doors with a
macro solution; `door topology` says `strict` (wirable) for all of them too. Neither excludes a
known door; but `families --check` prints "topology: no" for c79 because it runs the heuristic in
the MECHANISM's own bounding box (no free cell for wiring): a misleading label, not a bound.

Other generator bounds the audit checks on every design (none excluded a known door alone):
paths of 1-2 unit moves without reversal, the center straight in 2, providers 1-2 behind / 2 ahead,
<= 16 pistons and <= 144 box cells (`MAXC`), a QC-armed rest refused (`Mech::new`; vjeux: intended),
pistons only (static frame stones are added after the solve by mechspace `--full-frame`; vjeux
2026-10-19: the 3 extras may MOVE, which `--full-frame`'s "stay air on the whole cycle" excludes),
mechspace's sub-model `--no-door-bodies` (no known door has a piston body in the 3x3).

## 3. Wiring tools

c79's wiring, mechanic by mechanic (ok / what is missing):

| mechanic in c79 | satwire | cosearch | boxproof |
|---|---|---|---|
| lever ON at rest | main: lever ON from rest is the encoding (`fn on`); `--lever-on` on explore/satwire 91730a2+ | main's screen needs an unpowered lever; fixed on explore/cosearch2 c85570e | rest lever off (`lever_state(lev, false)`): missing |
| lever on a block's front face | ok (`--lever-oop`) | main: in-plane only; explore/audit-cslever f6de2a2 | ok (`--oop`) |
| droppers (4,3), (0,-3) as immovable conductors | ok (dropper kind) | ok (facing south = up/down/east/west by equivalence: an empty dropper's facing changes nothing) | no dropper cell (diff.rs `cell_of_state`) |
| observers reading a dropper's triggered state | ok | ok | missing (no dropper) |
| dust steps (2) | ok (diagonal wire rules) | ok | ok |
| normal piston ND(3,-2) firing into an empty cell watched by an observer | from-wired: recorded as mechanism; plan mode: no pistons in the wiring palette | ok | ok |
| level-held pistons (SW+(4,1), SW+(4,2), SD+(1,5)) | ok | ok | ok |
| carried piston SD(1,3), TM moving 3 times, 3 static frame blocks | ok | ok | ok |

Across the archive and the gadgets (the 153 designs and gadgets whose blocks all lie in the door
plane; the 3D designs are trivially outside every one of these flat tools), what each tool cannot
hold, by number of designs:

| tool | cannot hold (designs) |
|---|---|
| satwire | static redstone blocks (21: stripped as wiring, not a wiring kind); out-of-plane facings: pistons (36), observers (10), repeaters (9); sculk sensors (5), rails / powered rails / detector rails (2 each), glazed terracotta, copper trapdoors: NOT wiring kinds and silently modeled as plain CONDUCTORS by `mk()` (wrong semantics, no error); lever ON at rest (4, main) |
| cosearch | front-face levers (15, main; explore/audit-cslever); out-of-plane facings: pistons (39), observers (10), repeaters (9), iron trapdoors (5); sculk sensors (5), rails and detector rails (no redsim model), powered rails (not in the palette); lever ON at rest (3, main) |
| boxproof | droppers (30), slime (7), iron trapdoors (5), comparators (5), copper bulbs (4), sculk sensors (5), rails (2), powered rails (2); out-of-plane facings: pistons (39), observers (10), repeaters (9); lever ON at rest (4) |

## 4. Hard-coded rules

Live `file:line` (the audit finds them by pattern each run; this table is its output on
explore/audit).

| rule | where (live) | what real designs it excludes |
|---|---|---|
| judge: door cells = 9 identical plain full blocks, no mechanism block | lib.rs:1003 | doors whose closed face mixes blocks (glass + stone), or uses a mechanism block (observer/slime) as a door block; the community allows any 'full block' set (rules: identical) |
| judge: lever port must hold a lever (either polarity since cc4686d) | lib.rs:1123 | buttons, pressure plates, or a repeater/observer input as the port (the community's input device is any switch) |
| judge: nothing in front of / behind the door at any distance | lib.rs:132, lib.rs:136, lib.rs:879, lib.rs:1109 | a lever or dust placed in the tunnel columns (vjeux's rule: intended) |
| judge: settle at placement (raw paste, wire shapes, one self update each, then idle) | lib.rs:481, lib.rs:1074, lib.rs:1345 | designs resting with a BUD- or QC-armed piston: INTENDED (vjeux 2026-10-19: no QC-armed piston at rest); a rest state that depends on placement order is buildable's job |
| judge: every block passes canSurvive at rest | lib.rs:1087 | nothing real (a floating attachment pops in survival) |
| judge: no random ticks (isRandomlyTicking) | lib.rs:340, lib.rs:612, lib.rs:682, lib.rs:1086 | unwaxed copper, leaves (non-persistent), crops: intended |
| judge: open = 9 door cells + walkway walkable, no moving block | lib.rs:1031 | doors that are open while a block is still moving there (bouncing allowed only between settled states) |
| judge: close = some closed state repeats an earlier one (PERIODIC rest, main 96a62d6; before: identical to the rest state) | lib.rs:1208, lib.rs:1251 | nothing now in the judge; doors whose internal state alternates were rejected before 96a62d6 |
| door ideal: periodic rest (FIXED explore/audit 9040a14; was exact rest after each close) | ideal.rs:313 | before 9040a14: periodic-rest mechanisms failed `door ideal` and every tool replaying plans with it (fullmech --verify-ideal, families --verify, satsweep validation) |
| props-dyn env_run: periodic rest, surroundings exact (FIXED explore/audit 5026010) | properties_dyn.rs:369 | before 5026010: a periodic-rest door failed every dynamic property (record eligibility, --eligible / --repeater-input / --seamless +hall searches) |
| local-search screen_in: periodic rest (FIXED explore/audit 38f4b6f) | search.rs:1418 | before 38f4b6f: periodic-rest candidates of minimize/regrow/grow/harvest were screened out before the judge saw them |
| redsim screen: periodic rest (FIXED explore/audit a4c5a84) | redsim_screen.rs:202 | before a4c5a84: same for the redsim pre-screen / redsim-verify |
| tile judge: periodic rest (FIXED explore/audit 610a8ab) | tile.rs:489 | before 610a8ab: periodic tile modules |
| STILL exact-rest: cosearch screen (plane compared after each close) | cosearch_search.rs:524 | periodic-rest candidates lose points in the annealing screen |
| STILL exact-rest: boxproof (close ends at exactly the rest state) | boxproof/bmc.rs:4 | periodic-rest doors are outside boxproof's space (its UNSATs do not cover them) |
| STILL exact-rest: satwire (both stable states fixed points, each trace ends where the other starts) | satwire.rs:28, satwire.rs:2335, satwire.rs:2338 | periodic-rest wirings (e.g. T-flip-flop phases) |
| STILL exact-rest: fullmech / mech-synth / families macro (close back to the exact rest state) | fullmech.rs:19 | mechanisms that close into a different but equivalent internal state (two-phase machines) |
| placement = video.rs::place (raw paste in block_list order) | video.rs:281, video.rs:364 | states only reachable by a build order (e.g. a piston extended without power): judged unstable |
| legality: FORBIDDEN list + no orphan piston heads | lib.rs:353, lib.rs:751 | nothing real (survival-only blocks) |
| category: solid = not gravity, not sculk, no entities | lib.rs:654 | a solid door with a sculk sensor anywhere is ranked 'sculk' even if the sensor is unused |
| flat label (single_layer): every block in the door plane, the lever cell ignored (main d975c15) | lib.rs:374 | fixed: before d975c15 the community 7x9 as built (lever on (4,5)'s front face) was 7x9x2, not flat, off the flat boards and out of flat-only searches |
| cosearch search grid: the lever in the door plane only (main; front/back-face levers on explore/audit-cslever f6de2a2) | cosearch_search.rs:144, cosearch_search.rs:515, cosearch_search.rs:1083 | c79's real lever (front face of (4,5)): a flat-by-rule design main's cosearch cannot seed or move |
| redsim judge: falling blocks are full cubes (FIXED explore/audit 088f15e) | cosearch.rs:90 | before 088f15e cosearch eval/search/agree rejected EVERY gravity door ('not closed at rest: door cell is minecraft:sand': 33/33 archived gravity doors) |
| acceptance positions: 4 + extra (default 28) everywhere, never fewer than the 4 default positions | search.rs:39 | a door that works at 1-3 positions (vjeux: a 1-position pass should count as a find) is never reported by minimize/grow/regrow/resynth/synth (min 4), cosearch (main: hard-coded 32, cosearch_search.rs extra_offsets(28)), satwire (--extra-offsets 28), families/fullmech ideal replays (4) |
| boxproof bmc: the door placed at dx = (w-3)/2, dy = (h-3)/2 unless --dx/--dy | boxproof/bmc.rs:463 | off-center placements (dx 0,1,3,4 in 7 wide; dy 3 in 8 tall): a 7x8 run covers 1 of 10 placements |
| mechspace --door-offset (the single-U sweep ran with 2,2 only) | mechspace.rs:1500 | off-center 7-wide / 8-tall placements of the c78 single-U sweep |
| flat-only seeds: cosearch search refuses non-flat seeds | cosearch_search.rs:894, cosearch_search.rs:1114 | fixed by d975c15 for front-face levers (single_layer ignores the lever) |
| frame cells must hold the DOOR block itself when open (d975c15); the 3 extras may move | sheet.rs:92 | frame-12 tests that add STATIC stone (sheet --fill, mechspace --full-frame: 'frame cells air when open stay air on the cycle') exclude doors whose 3 extra frame blocks move |
| mechspace --full-frame: frame cells left air when open must stay air on the whole cycle (static stones) | mechspace.rs:875, mechspace.rs:880, mechspace.rs:1976 | doors whose 3 extra frame blocks are moved into place (vjeux: the extras may move) |
| verify prints the bounding box with the lever | cli.rs:4262, cli.rs:4616 | the printed box (7x9x2) disagrees with the community volume (7x9x1, lever excluded) |
| props-dyn repeater rig + door properties' repeater input: polarity (FIXED explore/audit 4c483b9) | properties_dyn.rs:605 | before 4c483b9 every lever-ON door failed repeater input ('not open'): c79 was not record-eligible and --eligible / --repeater-input searches rejected its family; now ELIGIBLE (x32, 15/15) |
| cosearch screen: lever must be unpowered at rest (main) | cosearch_search.rs:516 | lever-ON seeds (c79) score as broken (max-1 points); fixed on explore/cosearch2 c85570e, not merged |
| minimize lever moves keep the design's polarity (FIXED explore/audit 9b1074d) | search.rs:646 | before 9b1074d every lever move of a lever-ON door turned its lever off and broke it |
| relever keeps the design's polarity (FIXED explore/audit 9b1074d) | relever.rs:77 | before 9b1074d a lever-ON door's relever variants were all broken |
| mech-wire: levers placed unpowered (its waves assume lever-off) | mechwire.rs:231 | wirings that need the lever's level at rest (level-held pullers released by the lever, c79's right side): a --lever-on option must invert every wave |
| satwire (main): lever ON from rest, OFF from open (--lever-on auto-detected on explore/satwire 91730a2+) | satwire.rs:1365 | lever-ON-at-rest wirings (c79: --from-wired --check diverges at t0, 157/485 piston-ticks) |
| satwire: lever candidates unpowered | satwire.rs:1109 | the same |
| boxproof: rest lever unpowered | boxproof/bmc.rs:355 | lever-ON doors |
| boxproof: cells = air/stone/obsidian/glass/note/lamp/rblock/dust/rep/obs/torch/piston | boxproof/diff.rs:65 | droppers (c79's conductors and sensors), trapdoors, slabs, comparators, bulbs, leaves |
| families: 1-move exits use RETRACTED sticky pullers (unless --ext-pullers, mechspace only) | families.rs:1581 | level-held pullers released by the lever (c79's MR/TR, SW+(4,1) SW+(4,2)); fullmech and the catalog never set the flag |
| families: paths of 1-2 unit moves, no reversal | families.rs:1474 | a door block moving 3 times or back (c79's TM: down, up, up) - only harmless when the pieces are generated for another nominal path |
| families: the center leaves straight in exactly 2 moves | families.rs:1521 | a center that exits in 1 move sideways, 3 moves, or bends |
| families: providers = a sticky puller 2 ahead or a pusher 1-2 behind | families.rs:1353 | a block pushed through a chain from 3+ cells (c79's BM is pushed from (1,3) through TM and C; generated anyway via its (1,-2) pull) |
| families: pistons only (no stone, obsidian, slime, droppers, carried sources) in generated sets | families.rs:1324 | static frame stones (frame 12/12: mechspace --full-frame adds them after the solve), helper blocks, carried redstone blocks / observers |
| families: a piston held on at rest above a retracted piston is refused (QC-armed rest) | families.rs:258 | rests where the QC-armed piston is meant to stay (blocked) or fire on the first update |
| families: box <= 144 cells, <= 16 pistons | families.rs:35 | boxes over 12x12 |
| families: default P <= 10 (catalog), mechspace 11, fullmech 10 | cli.rs:1111, cli.rs:1871 | mechanisms with 11+ pistons (c79 has 10 mechanism pistons + 1 signal piston: families --check counts 11) |
| families/mechspace hole rule (swept ring cells air when open) | families.rs:21, families.rs:615, families.rs:617, families.rs:801 | replaced by frame 12/12 (vjeux 03:36Z) |
| mechspace sub-model: no piston body inside the 3x3 | mechspace.rs:314, mechspace.rs:317 | doors whose piston bodies cross the opening (none known among working flat doors; see the audit table) |
| topology heuristic (parity, regions); `families --check` runs it in the MECHANISM's own box | families.rs:2331 | label: `families --check` prints 'topology: no' for c79 (no free cell in the mechanism's bounding box); in each door's own build box topology says 'strict' for c79 and all 66 flat archived doors |
| connect: only the lever's region awake at each flip; regions = free cells outside the footprint | connect.rs:447 | no known door: WIRABLE for c79 (real 7x9 placement; NOT in 7x8 or 7x9 shifted up) and all 66 flat archived doors in their own boxes |
| sheet piston count excludes heads (FIXED explore/audit add7f90) | sheet.rs:308 | label only: c79 printed '14 pistons' (11 + 3 heads) |
| sheet (designs): flip 1 = open | sheet.rs:286 | a design file of an OPEN state (c79-open-b) draws 'frame 3/12' (door cycle handles it) |
| properties CLEAN: never moves back | properties.rs:872 | label: c79's TM moves down then up (correct per regulation 2.1.5.3) |

## 5. Silent exclusions to check (coordinator's list), tool by tool

| item | judge | local search (minimize...) | families / mechspace / fullmech | connect | satwire | cosearch | boxproof | cheapest addition |
|---|---|---|---|---|---|---|---|---|
| off-center door placements (dx 1/3 in 7 wide, every dy in 8 tall) | - | - | families: every offset; mechspace `--door-offset` (the single-U sweep ran 2,2 only) | every placement | any `--box` | any `--box` | CENTERED by default (dx = (w-3)/2, dy = (h-3)/2): a 7x8 run covers 1 of 10 placements | boxproof: sweep `--dx/--dy` (a loop in batch); mechspace: rerun single-U without `--door-offset` |
| frame cells at rest used as wiring space | ok | ok | ok (catalog column `rest-ring` only) | ring cells are never FREE (only hop stones): a wire, dust or component in a ring cell at rest is excluded | `--forbid-ring` / `--hall-ring` optional | ok | `--ring-rest` optional | connect: let unswept ring cells hold wiring at rest (they need a plain block only when open) |
| 0-tick moves, block dropping (spit) | ok | ok | families/mechspace: no (2 gt pulse + spit only); fullmech: 1 gt pulses, 0-ticks only through helpers | no (macro events) | no (observers miss intra-tick transients) | ok (redsim) | ~ 2 rounds | - (model work) |
| a piston fired mid-cycle by a later block update (BUD) | ok | ok | QC coupling only | no | ~ the firing update is assumed | ok | ok (exact update patterns) | - |
| movable helper blocks, double extenders | ok | ok | no (pistons only; `fullmech --anneal --helpers` samples) | no | - | ok | ok | families: a helper piece type in `providers` |
| powered / activator rails, container circuits (dropper / hopper / comparator memory) | ok | ok (exotic palettes: rail, arail, disp, dropper, crafter) | - | - | rails MIS-MODELED as conductors; containers no | no (rails not modeled by its palette, droppers empty only) | no | cosearch palette families `rail`, `hopper`, comparator reading containers (redsim already models them) |
| internal state alternating between cycles (periodic) | ok (96a62d6) | ok (38f4b6f, a4c5a84) | exact rest in the macro model | exact rest | fixed points | screen compares the plane after each close | close = exact rest | cosearch screen: periodic like the judge (on cosearch2's file) |
| a door working at only some positions / rotations | 4 positions minimum | acceptance 4 + `--extra-offsets` (default 28), never fewer than 4 | ideal replays at 4 | - | `--extra-offsets` 28 | main: 32 hard-coded (`extra_offsets(28)`), cosearch2: `--judge-extra` | redsim screen then judge | a `--positions N` (first N of the judge's offsets, N >= 1) on verify and every search, and a "works at k of 32" report instead of a rejection |
| sculk sensors | ok (category sculk) | ok (`--to-sculk`) | no | no | modeled as a CONDUCTOR (wrong) | no redsim model | no | (vjeux deciding) |
| gravity door blocks | ok (category gravity) | ok | no gravity in the macro model | no | ? | ok since 088f15e (was: every gravity door rejected) | no | (vjeux deciding) |

## 6. Handed off (one line each)

* satwire: `--lever-on` (done on explore/satwire 91730a2+, auto-detected); still: `mk()` treats unknown blocks (sculk, rails, glazed terracotta, copper trapdoors) as plain conductors silently: make them an error or model them; static redstone blocks as a wiring kind.
* boxproof: a dropper cell (immovable conductor whose `triggered` state observers watch) and a rest-lever-ON option; sweep door placements (`--dx/--dy`) before any UNSAT claim (default = centered only). Admits c79.
* families / mechspace / fullmech: (a) reuse a piston EXTENDED at rest as a lifted / carried provider (drop `!ext` in the lift and carry branches; it retracts first); (b) let a carried-only piston be placed (seed the reuse candidates with pistons at 1-2 cells of each provider position, not only placed ones); (c) `--ext-pullers` in the catalog and in fullmech. Admit fast-clean / speed-wire (39 files), hA-full-rl-dropper / eligible-s4ff / fresh-a-sf1-w2 (19), c79.
* mechspace `--full-frame`: the 3 extra frame blocks may move (vjeux 2026-10-19): allow a frame cell that is swept on the cycle if it ends filled with the door block.
* mech-wire: a `--lever-on` mode (the lever's level held at rest; every wave inverted). Admits level-held designs like c79's right side.
* cosearch (cosearch2): screen periodic rest; the screen's frame check should require the door block itself (d975c15); merge explore/audit-cslever (off-plane lever + polarity-keeping lever moves).
* connect: unswept ring cells usable as wiring at rest (no known door needs it yet: connect accepts all 66 flat archived doors).
* `families --check`: run topology / wire in a box with room for wiring (`--check-margin`), or say "in the mechanism box" (prints "topology: no" for c79).
* every search: `--positions N` / partial-position reports (a 1-position pass counts as a find).
* `door verify`: print the volume box without the lever (prints 7x9x2 for c79 with its real lever).
