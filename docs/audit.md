# Audit: what our rules, checks, generators and wiring tools silently exclude

Every "none found" is bounded by a tool, and three times now vjeux showed us a community design
that one of our bounds had silently excluded (lever polarity, the open frame, extended-at-rest
pistons, a door block moving 3 times, droppers, normal pistons as signals). This page finds the
next ones mechanically, on doors that WORK: the community 7x9 (`tracks/c79/c79-closed.txt`, and
`tracks/audit/c79-closed-ff.txt` = the same door with its real lever on (4,5)'s front face), every
design in `archive/` and the gadget library. Track notebook:
[tracks/audit/NOTES.md](../designs/piston-door-3x3/tracks/audit/NOTES.md).

```
mctas door audit [designs or folders...] [--sections checks,mech,wiring,rules] [--jvm] --out F [--tsv MECH.tsv]
mctas door audit --explain <design>      # one door: paths, every move and who made it, membership, bounds
```

Generated reports (never edit them by hand, rerun the command):
[tracks/audit/AUDIT.md](../designs/piston-door-3x3/tracks/audit/AUDIT.md) (sections 2-4 for every
archived design and gadget), [tracks/audit/CHECKS-c79.md](../designs/piston-door-3x3/tracks/audit/CHECKS-c79.md)
(section 1 with the JVM checks), [audit-rules.md](audit-rules.md) (section 4, live `file:line`),
`tracks/audit/MECH.tsv` (the extracted flat mechanisms and their macro runs in families' catalog
format, readable by `door connect`, `door topology`, `boxproof catalog`).

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
| 2947ab6, 71a84b2 + later | `door audit` itself, `families::generate_within` / `explain_within` (no change to the generator's output) | - |
| explore/audit-cslever f6de2a2 | cosearch search/pockets: a lever hung on a block's front/back face | c79 with its real lever as a seed: 5 finds in 20 s, JVM WORKS (main took pockets' parallel implementation instead, 3d3f5c27) |

Fixed on main by the owning tracks after this audit's reports (re-checked on main dfe893a):

| where | what | check |
|---|---|---|
| satwire (main) | lever polarity auto-detected (`lever_on_rest`), written levers keep it, static redstone blocks placeable | `satwire c79-closed.txt --from-wired --check`: 157/485 piston-tick mismatches -> 0/482 |
| boxproof fd8c52b3 | a dropper cell, a rest-lever-ON option, comparators, copper bulbs | c79's blocks are all boxproof cells now |
| cosearch (pockets 3d3f5c27, cosearch2 c85570e) | either lever polarity in the screen, periodic rest in the screen, a lever on a block's front/back face (`Grid.lever_dz`) | c79 with its real lever seeds and screens |
| mechspace 4f8f0604 | movable frame extras (rule A, `try_movable`) | vjeux: the 3 extras may move |

## 1. Checks on the community 7x9

c79 is a valid community door: everything vjeux did not forbid must pass. Before -> after this
track's fixes (and main's d975c15 / 96a62d6). Full output: CHECKS-c79.md.

| check | c79-closed (lever on top of (4,5), flat file) | c79 with its real front-face lever |
|---|---|---|
| `door verify` (real judge, 4 positions) | WORKS 51 blocks, 14/14, rotation-safe, flat | WORKS; was "7x9x2", not flat -> flat since d975c15 (verify still PRINTS 7x9x2: the box counts the lever) |
| `cosearch eval` (redsim judge, 32 x 3) | WORKS, FULL, hallway ok, 63b (7x9x1) | same |
| `door properties` | FULL, volume 63 (7x9x1), lever on the surface; CLEAN: no ("door block 1 moves back": TM goes down then up, correct per 2.1.5.3) | same |
| `door properties-dyn` | hallway, contained (6 fills), snow, steps, 20 cycles yes; repeater input NO -> yes 15/15 (4c483b9) | same |
| `door eligible` | not eligible (repeater) -> ELIGIBLE (judge x32 + repeater x32) | same |
| `door cycle` | CYCLES (6 flips x 4 positions, frame 12/12 open) | same |
| `door sheet --fill` | frame 12/12; "14 pistons" -> 11 (add7f90) | - |
| leaderboard boards | solid, slime-free, flat, 3-thick wall, seamless FULL + hallway; record-eligible only after 4c483b9; not "on the ground" (blocks below the door) | flat board only since d975c15 |
| minimize lever moves / `door relever` | placed unpowered levers (broke the door) -> keep polarity (9b1074d) | same |
| cosearch search screen | main before the merges: required `Lever { powered: false }`; now either polarity (cosearch2 c85570e) | main before: grid refused the off-plane lever; now `Grid.lever_dz` (pockets 3d3f5c27) |
| `satwire --from-wired --check` | before: diverged at t0 on the lever chain, 157/485 piston-ticks; main now: 0/482 (lever polarity auto-detected) | same (+ `--lever-oop`) |
| boxproof bmc | before: no dropper cell, rest lever off; main fd8c52b3: both added | same |
| `families --check` / `fullmech --check` (solvers) | both solve c79's piece set (families: open 11 + close 9, sym 12 with levels; fullmech: open 10 + close 9) | - |
| `door topology` (heuristic) | `families --check` prints "topology: no" (it runs in the mechanism's own box, no room for wiring); in c79's own 7x9 box: `strict` (wirable) | - |
| `door connect` (region model) | c79's mechanism WIRABLE in its real 7x9 placement (x -2..4, y -3..5); NOT in 7x8 or 7x9 shifted up | - |
| c79-open-b (the screenshot, OPEN state) | judge "not closed at rest" (correct: the file rests open); `door sheet` draws flip 1 as open: "frame 3/12" (label; `door cycle` handles it) | - |

## 2. Mechanism generators

Columns: g1 families v3 (catalog: P <= 10, two 1, lifts, pushes; fullmech uses the same
generator), g2 mechspace gen default (P <= 11), g3 mechspace `--ext-pullers`, g6 every flag (P <= 16,
two 7, carry 2, vmid any, ext-pullers). Archive (main dfe893a): 189 designs with a flat mechanism;
IN under g1 / g2: 95, under g3..g6: 99 (+ c79, c79-ff and the two archived 63b c79 doors); 65 OUT
under every flag (the mechanisms below + 2 gadget rows).

| door (files) | mechanism | g1 | g2 | g3 | g6 | the exact bound that excludes it |
|---|---|---|---|---|---|---|
| c79, c79-ff | 10 pistons: SE(-2,0..2) SW+(4,1) SW+(4,2) SE(-1,-1) SU(1,-2) SU(2,-2) SD(1,3) SD+(1,5) (+ signal ND(3,-2)); static frame (0,3) (2,3) (3,0); mechanism box 7x8 (the volume's 7x9 comes from the y=-3 wiring row) | out | out | IN | IN | 1-move exits use RETRACTED sticky pullers only (`Gen::others`, families.rs `if ext && !EXT_PULLERS`): MR/TR leave pulled by SW+(4,1)/(4,2) extended at rest. `--ext-pullers` exists only in mechspace; the catalog and fullmech never set it. Harmless path-level mismatches: BM is pushed from 3 behind through TM and C (providers: 1-2 behind), TM moves 3 times and back (paths: 1-2 moves, no reversal); the set is generated anyway via other nominal paths |
| fast-clean (21) + speed-wire (18): a1c, s4c, n3c, c1r, o2... | 11: SU+(1,-2) SE+(-2,0..2) SW+(4,0..2) SE(-1,3) SD(0,3) NW/SW(3,3) SD+(1,4) | out | out | out | out | a piston that only acts AWAY from its rest cell is never placed: SD(0,3) is pushed sideways to (1,3) by SE(-1,3) and only then pulls C; `Gen::providers` reuses (lifts / carries) only pieces an earlier provider already placed, and (0,3) is no provider position |
| hA-full-rl-dropper (14), eligible-s4ff (3), fresh-a-sf1-w2 (2) | 10: SU(1,-2) SE/SW(-2/4,0..2) SE+(-1,3) or SW(3,3), SD+(1,4) SD(2,4)/SD(1,5) | out | out | out | out | a piston EXTENDED at rest that is later lifted / carried is never reused: SD+(1,4) retracts, is pushed down to (1,3) by SD(1,5), then pulls C; lifts and `--carry` require `!ext` (`Gen::providers`, families.rs:1400/1406/1413) |
| gravity-sf1 (5) | 9 pistons, sand door blocks | out | out | out | out | door blocks FALL (BM (1,0)->(1,-2), C (1,1)->(1,-1)->(1,-3)): families' macro model has no gravity blocks |

Solvers vs generators: the macro solver (`families::solve`) and the exact solver
(`fullmech --check`) both solve c79's piece set; only the GENERATORS exclude it. Every OUT row above
is a generator bound, not a solver bound.

Wiring-feasibility models on known doors (the audit runs both on each door's own mechanism in the
door's own build box): `door connect` says WIRABLE for c79 and for all 68 flat archived doors with a
macro solution; `door topology` says `strict` (wirable) for all of them too. Neither excludes a
known door; but `families --check` prints "topology: no" for c79 because it runs the heuristic in
the MECHANISM's own bounding box (no free cell for wiring): a misleading label, not a bound.

Other generator bounds the audit checks on every design (none excluded a known door alone):
paths of 1-2 unit moves without reversal, the center straight in 2, providers 1-2 behind / 2 ahead,
<= 16 pistons and <= 144 box cells (`MAXC`), a QC-armed rest refused (`Mech::new`; vjeux: intended),
pistons only (static frame stones are added after the solve by mechspace `--full-frame`, movable
since 4f8f0604), mechspace's sub-model `--no-door-bodies` (no known door has a piston body in the
3x3).

## 3. Wiring tools

c79's wiring, mechanic by mechanic (state of main dfe893a; "was" = when this audit started):

| mechanic in c79 | satwire | cosearch | boxproof |
|---|---|---|---|
| lever ON at rest | ok (was: the encoding WAS lever ON from rest, `fn on`; now `lever_on_rest`) | ok (was: the screen required an unpowered lever) | ok (was: rest lever off; fd8c52b3) |
| lever on a block's front face | ok (`--lever-oop`) | ok (was: in-plane only; pockets 3d3f5c27) | ok (`--oop`) |
| droppers (4,3), (0,-3) as immovable conductors | ok (dropper kind) | ok (facing south = up/down/east/west by equivalence: an empty dropper's facing changes nothing) | ok (was: no dropper cell; fd8c52b3) |
| observers reading a dropper's triggered state | ok | ok | ok (fd8c52b3) |
| dust steps (2) | ok (diagonal wire rules) | ok | ok |
| normal piston ND(3,-2) firing into an empty cell watched by an observer | from-wired: recorded as mechanism; plan mode: no pistons in the wiring palette | ok | ok |
| level-held pistons (SW+(4,1), SW+(4,2), SD+(1,5)) | ok | ok | ok |
| carried piston SD(1,3), TM moving 3 times, 3 static frame blocks | ok | ok | ok |

Across the archive and the gadgets (the 155 designs and gadgets whose blocks all lie in the door
plane; the 3D designs are trivially outside every one of these flat tools), what each tool still
cannot hold on main, by number of designs:

| tool | cannot hold (designs) |
|---|---|
| satwire | out-of-plane facings: pistons (36), observers (10), repeaters (9); NOT wiring kinds and silently modeled as plain CONDUCTORS by `mk()` (wrong semantics, no error): sculk sensors (5), rails / powered rails / detector rails (2 each), buttons, pressure plates, skulls, iron doors, fence gates, crafters, scaffolding, glazed terracotta (never pulled), copper trapdoors (1 each); targets stripped but not placeable (1) |
| cosearch | out-of-plane facings: pistons (39), observers (10), repeaters (9), iron trapdoors (5); sculk sensors (5), powered rails (2), buttons, skulls, fence gates, iron doors, scaffolding, crafters, honey, dispensers (not in the palette); rails, detector rails, pressure plates, copper trapdoors (no redsim model) |
| boxproof | slime (7), iron trapdoors (5), sculk sensors (5), rails / powered rails / detector rails (2 each), logs and leaves (2), buttons, pressure plates, skulls, doors, fence gates, crafters, dispensers, targets, scaffolding (1 each); out-of-plane facings: pistons (39), observers (10), repeaters (9) |

## 4. Hard-coded rules

All 55 rules with their live `file:line` and what real designs each excludes, including the ones
fixed since: [audit-rules.md](audit-rules.md). The ones still open on main dfe893a + explore/audit:

| rule | where | what real designs it excludes |
|---|---|---|
| families: a lifted / carried provider must be RETRACTED at rest | families.rs:1400, 1406, 1413 | 19 archived flat doors under every flag (hA-full-rl-dropper, eligible-s4ff, fresh-a-sf1-w2) |
| families: only pieces an earlier provider PLACED are reused | families.rs:1399 | 39 archived flat doors under every flag (fast-clean, speed-wire) |
| families: 1-move exits use RETRACTED pullers unless `--ext-pullers` (mechspace only) | families.rs:1581 | c79 and the 63b c79 doors under the catalog and fullmech |
| families: paths of 1-2 unit moves, center straight in 2, providers 1-2 behind / 2 ahead, pistons only, P <= 10 / 11 / 10 defaults | families.rs:1474, 1521, 1353, 1324; door-mech/src/lib.rs:283, 526 | none alone among known doors (c79's TM 3 moves and BM chain push are generated via other paths) |
| close = EXACT rest | boxproof/bmc.rs:4, satwire.rs:28, fullmech.rs:19 | periodic-rest doors (the judge accepts them since 96a62d6): boxproof UNSATs and satwire / fullmech / families searches do not cover them |
| acceptance needs the 4 default positions at least | search.rs:39 | a door working at 1-3 positions (vjeux: a 1-position pass counts as a find): no search reports it |
| boxproof bmc: the door CENTERED unless `--dx/--dy` | boxproof/bmc.rs:548 | off-center placements (dx 0,1,3,4 in 7 wide; dy 3 in 8 tall): a 7x8 run covers 1 of 10 |
| mechspace single-U sweep with `--door-offset 2,2` only | mechspace.rs:1732 | off-center 7-wide / 8-tall single-U doors |
| mech-wire: levers placed unpowered | mechwire.rs:231 | lever-ON wirings (level-held pullers released by the lever, c79's right side) |
| category solid = no sculk anywhere | lib.rs:654 | a solid door with an unused sculk sensor is ranked 'sculk' |
| verify prints the box with the lever | door-judge/src/lib.rs:1027, 1134 | label: c79 with its real lever prints 7x9x2 (the community volume is 7x9x1) |
| `door sheet` treats flip 1 as open | sheet.rs:286 | label: an OPEN design file (c79-open-b) draws "frame 3/12" |
| judge: lever port must hold a lever | lib.rs:1123 | buttons, pressure plates, repeater / observer inputs as the port (the community's input is any switch) |

## 5. Silent exclusions to check (coordinator's list), tool by tool

| item | judge | local search (minimize...) | families / mechspace / fullmech | connect | satwire | cosearch | boxproof | cheapest addition |
|---|---|---|---|---|---|---|---|---|
| off-center door placements (dx 1/3 in 7 wide, every dy in 8 tall) | - | - | families: every offset; mechspace `--door-offset` (the single-U sweep ran 2,2 only) | every placement | any `--box` | any `--box` | CENTERED by default (dx = (w-3)/2, dy = (h-3)/2): a 7x8 run covers 1 of 10 placements | boxproof: sweep `--dx/--dy` (a loop in batch); mechspace: rerun single-U without `--door-offset` |
| frame cells at rest used as wiring space | ok | ok | `MECHSPACE_FRAME_SLOTS` (pistons in frame cells at rest, 4f8f0604) | ring cells are never FREE (only hop stones): a wire, dust or component in a ring cell at rest is excluded | `--forbid-ring` / `--hall-ring` optional | ok | `--ring-rest` optional | connect: let unswept ring cells hold wiring at rest (no known door needs it: connect accepts all 68) |
| 0-tick moves, block dropping (spit) | ok | ok | families/mechspace: no (2 gt pulse + spit only); fullmech: 1 gt pulses, 0-ticks only through helpers | no (macro events) | no (observers miss intra-tick transients) | ok (redsim) | ~ 2 rounds | - (model work) |
| a piston fired mid-cycle by a later block update (BUD) | ok | ok | QC coupling only | no | ~ the firing update is assumed | ok | ok (exact update patterns) | - |
| movable helper blocks, double extenders | ok | ok | no (pistons only; `fullmech --anneal --helpers` samples) | no | - | ok | ok | families: a helper piece type in `providers` |
| powered / activator rails, container circuits (dropper / hopper / comparator memory) | ok | ok (exotic palettes: rail, arail, disp, dropper, crafter) | - | - | rails MIS-MODELED as conductors; containers no | no (rails not modeled by its palette, droppers empty only) | no | cosearch palette families `rail`, `hopper`, comparator reading containers (redsim already models them) |
| internal state alternating between cycles (periodic) | ok (96a62d6) | ok (38f4b6f, a4c5a84) | exact rest in the macro model | exact rest | fixed points | ok (pockets 3d3f5c27) | close = exact rest | boxproof: accept a repeated closed state; satwire: hard (fixed points are its encoding) |
| a door working at only some positions / rotations | 4 positions minimum | acceptance 4 + `--extra-offsets` (default 28), never fewer than 4 | ideal replays at 4 | - | `--extra-offsets` 28 | `--judge-extra` (default 28) | redsim screen then judge | a `--positions N` (first N of the judge's offsets, N >= 1) on verify and every search, and a "works at k of 32" report instead of a rejection |
| sculk sensors | ok (category sculk) | ok (`--to-sculk`) | no | no | modeled as a CONDUCTOR (wrong) | no redsim model | no | (vjeux deciding) |
| gravity door blocks | ok (category gravity) | ok | no gravity in the macro model | no | ? | ok since 088f15e (was: every gravity door rejected) | no | (vjeux deciding) |

## 6. Handed off (one line each; done ones removed)

* families / mechspace / fullmech: (a) reuse a piston EXTENDED at rest as a lifted / carried provider (drop `!ext` in the lift and carry branches; it retracts first); (b) let a carried-only piston be placed (seed the reuse candidates with pistons 1-2 cells beside each provider position, not only placed ones); (c) `--ext-pullers` in the catalog and in fullmech. Admits fast-clean / speed-wire (39 files), hA-full-rl-dropper / eligible-s4ff / fresh-a-sf1-w2 (19), c79.
* satwire: `mk()` models unknown blocks (sculk, rails, buttons, pressure plates, glazed terracotta, copper trapdoors...) as plain conductors without a word: make them an error or model them.
* boxproof: sweep door placements (`--dx/--dy`) before an UNSAT claim (default = centered only); a periodic-rest close.
* mech-wire: a `--lever-on` mode (the lever's level held at rest; every wave inverted).
* `families --check`: run topology / wire in a box with room for wiring, or say "in the mechanism box" (prints "topology: no" for c79).
* every search: `--positions N` / partial-position reports (a 1-position pass counts as a find).
* `door verify`: print the volume box without the lever (prints 7x9x2 for c79 with its real lever).
