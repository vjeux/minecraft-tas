# props: Redstone Squid catalogue properties of every door

vjeux (2026-10-18): "Could you check what properties all our doors have?" against the community's
piston door rules (Redstone Squid records catalogue, sections 2.1.4-2.1.7, 3 and 4; the rules text
is data in ~/agents/squid-door-rules.txt on the box).

Tool: `mctas door properties [<designs...>] [--archive-dir designs/piston-door-3x3/archive]
[--workers N] [--out-md f] [--out-tsv f] [--redo]` (crates/piston-door/src/properties.rs).
Without designs it takes every working design of the leaderboard's list (HEAD + explore/*,
verdicts from archive/verdicts-v5.tsv; designs not judged yet are skipped). Results are cached by
design hash in archive/properties-v1.tsv. The full table: [PROPERTIES.md](PROPERTIES.md) (TSV:
properties.tsv, with the "why" columns).

Per vjeux (coordinator, 18:45Z): every check runs at ONE position (the design as written), one
orientation, one cycle; NOT DIRECTIONAL and NOT LOCATIONAL come from the leaderboard's cached
verdicts (rotation check; 32-position judge). The dynamic checks (hallway-compatible, contained,
snow, repeater input, player steps, 20 cycles) moved to the `props-dyn` agent; this tool has them
behind `--dynamic` (same definitions as below) but they are not in the table yet.

## How a cycle is read

`record`: the design is placed exactly as the judge places it (raw blocks, wire shapes, one
self-update each, settle), then the lever is flipped by the player (`use`, between ticks) and the
world is stepped ONE tick at a time until idle, capturing after every tick the watch box (design +
walkway + 2) AND every moving block entity (the block it carries, its piston's facing, extending or
retracting: new `Worker::get_region_moving`). Same for the close.

Door blocks are TRACKED: a moving block entity seen for the first time moved its block from
`pos - motion` to `pos`; all moves of a tick apply at once (a pushed line shifts as a whole). A
move that starts and ends within one tick (a spit) leaves no moving block: a door block whose cell
no longer holds it is matched to an unexplained door-type block 1-2 cells away along an axis. The
table reports `track=lost N` if a door block was lost (none so far is the goal).

## Geometry

* Door plane = `z = door.z` (`x = door.x` for `door-axis: z`); depth = distance along that axis.
* T (tunnel) = the 3x3 columns through the opening, any depth (the frame cells included).
* S (hallway surface) = the cells face-adjacent to T and not in T: 12 per depth slice (floor
  `y = door.y-1`, ceiling `y = door.y+3`, walls), the door plane included.
* CIRCUITRY in a frame = every non-air block except the 9 door blocks (tracked), the lever, and
  quartz-like blocks resting in S (they read as hallway wall). Moving non-door blocks and piston
  heads are circuitry.
* Quartz-like = a whitelist of opaque, full, conductive, pushable, non-gravity building blocks
  (stone family, cobblestone, planks, wool, concrete, terracotta, logs, waxed copper...).
  Judgment call: glazed terracotta is NOT quartz-like (designs use it exactly where slime must not
  drag a block; quartz there would break them).

## Properties (column: definition)

* **seamless** (2.1.4.1): QUART <side> = closed, no circuitry in T or S strictly on that side
  (`z-` = the viewer stands at depth < 0). SEMI <side> = QUART <side> + open (settled) state: no
  circuitry in T or S at any depth, door plane included. FULL = QUART on both sides + open clean.
  SUPER = FULL + every tick of opening and closing clean (moving non-door blocks, piston heads count).
  Closed checks skip the door plane's S ring: when closed its only face toward T touches a door
  block, so it cannot be seen. `seamless_why` (TSV) = the first cell that stops the next tier
  (state + block + position). NOT YET combined with hallway-compatible (props-dyn): a seamless
  claim also needs the hallway to be buildable.
* **dents** (2.1.4.2-3), both stable states: in T, a colliding block = `inelig` (hallway blocked),
  a non-colliding one = dent, or minor if it stands on/hangs from an S cell (floor dust, wall
  torch, a lever on the hallway wall). In S, a full block (any) = fine (flat), a partial or
  non-colliding block (piston head, slab, dust, lever...) = recession: minor on walls/ceiling,
  dent on the floor; an S cell filled at rest but empty when open = a hole (same rule). Empty S
  cells are assumed filled by the hallway (props-dyn checks that).
* **full / semi trap/flush/deluxe** (2.1.4.4), per viewer side, both stable states: every
  circuitry block at depth >= 0 / >= 1 / >= 2 behind the door plane (seen from that side);
  semi also allows circuitry at or below the floor (y <= door.y-1). Lists the best tier per side.
* **hipster** (2.1.4.5), both stable states: FULL floor/ceiling/wall = all circuitry beyond S
  (y <= door.y-2 / y >= door.y+4 / two or more columns beside the opening, either side);
  SEMI = within S or beyond (y <= door.y-1 / y >= door.y+3 / one column beside).
* **sym axes** (2.1.5.1): the T slices within the walkway depth (-3..3) as a 3x3 grid of empty /
  resting / moving, at rest and every tick of opening and closing; V (left-right), H (top-bottom),
  D1, D2 (diagonals) = the axes under which every grid is its own mirror image. Movements in S or
  behind it are ignored, as the rules say ("excludes any block movements on or behind the hallway").
* **sync** (2.1.5.2): sections = the 8 ring blocks and the center. SEMI = symmetrical (1 axis;
  the "last section in unison" alternative is trivial for a 1-block center). FULL = all 4 axes +
  the 8 ring blocks in motion at exactly the same ticks, opening and closing. SUPER = FULL + minimal
  motions (each ring block moves once, the center at most twice, per operation).
* **clean** (2.1.5.3): SUPER seamless + necessary movements only: each door block leaves the frame
  once while opening and enters it once while closing (no re-crossing: the bouncing doors fail) and
  never moves back to a cell it already occupied.
* **components** (2.1.6), codes that hold: S no slime, H no honey, G no gravity blocks (sand,
  gravel, concrete powder, anvils, dripstone, scaffolding, dragon egg), K no sticky surfaces (no
  sticky piston, slime or honey), O no observers, N no note blocks, E no entities, F no flying
  machines (no piston or observer carried two cells the same way in one operation), TD torch and
  dust only, RB redstone block only, ZA zomba- (quartz-like, pistons, hoppers, droppers,
  comparators), ZI zombi- (quartz-like, pistons, cauldrons, comparators); the lever is allowed
  everywhere. NO CLOCKS holds for every working door (the judge requires the build to go idle).
* **not dir.** (2.1.7.4): the leaderboard's rotation verdict (4 turns, 32 positions).
* **not loc.**: the cached judge verdict at 32 positions.
* **26.3 vanilla** (2.1.7.5, UP-TO-DATE): PASS / DIVERGES from the vanilla-check track's logs,
  matched by design hash; `sim only` when the design was never vanilla-checked.
* **lever surf.** (3.1.2): some axis ray from the lever (not into its support) leaves the volume
  box through air in both stable states. `lever_support` (TSV): the support cell holds the same
  block in every frame (3.1.5).
* **volume** (4.2): bounding box of the circuitry over the rest state, every tick of opening and
  closing, and the open state; door blocks, the frame cells and the lever excluded; T cells only in
  the open state. Printed as volume (width x height x depth).
* **player o/ov/c/cv** (4.3.1, player input, design position): opening time (T walkway + frame
  walkable for good = the judge's open), opening visible time (last tick a T or S cell moves or
  changes), closing time (door cells back for good), closing visible time. Repeater-input timings
  come from props-dyn.
* `risky` (TSV): blocks a player or the world can trigger (sculk sensors, pressure plates,
  tripwire, daylight detectors, targets, lightning rods): input to 3.8/3.9 (props-dyn tests them).

## Open judgment calls

* Seamless side: rules say "front side of closed door blocks"; we report every side that passes.
* Symmetry uses occupancy (empty/resting/moving), not block identity or motion direction.
* SUPER SYNC's "supreme layout" is not modeled (no named layouts for 3x3 here).
* Volume: quartz-like blocks resting in S are hallway, never circuitry (also when they are part of
  a moving structure at rest); a moving quartz-like block is circuitry while it moves.
* EXPANDABLE / TILEABLE / M WIDE / N HIGH: not computed (tileable is skipped per the coordinator).

## Joined columns from props-dyn

`mctas door properties` reads `archive/properties-dyn-v1.tsv` (props-dyn's dynamic checks, same
design hash) and shows hallway, contained (+ the shells that fail and the 1-block-gap variant),
weatherproof (= props-dyn's snow check passes AND no flammable block under the open sky),
repeater input (`n/a (no site)` = no free side of the lever's support for a repeater rig), steps,
uses (20 cycles). Derived here:
* isolated (3.9): `no` for sculk doors that hear footsteps; `?` when some other risky block
  (pressure plate, tripwire, daylight detector, target, lightning rod) is present; else yes.
* record-eligible (3): lever on the surface, steady support, repeater input works, 20 cycles,
  footsteps harmless, isolated, hallway-compatible and a seamless tier. The column lists what is
  missing.
* lever support: the same BLOCK (its state may change: a lamp lights) in every frame, never moved.

## Findings (2026-10-18 19:35Z, 248 working designs; props-dyn @947d8c9)

props-dyn redefined hallway-compatible (coordinator): the hallway fills only the ring cells that
are air at every tick of a cycle. A ring cell some block passes through therefore stays empty for
good: this tool now counts it as a HOLE (a missing hallway block: minor on walls and ceiling, a
dent on the floor; hidden in the door plane while closed) and treats circuitry in the cell right
behind a hole (one further out from the tunnel) as visible for SEAMLESS (`holes` in the TSV).

* Seamless: SUPER 0, FULL 22, SEMI 0 (every door clean in the open state is also clean on both
  closed sides), QUART 206, none 20 (the holes changed no tier).
* Hallway-compatible: 100; with a seamless tier: 93 (smallest: flat-b-v7-44b, volume 80; b38, 38
  blocks); FULL SEAMLESS + hallway: 4 (flat-v2-57b smallest, 57 blocks, volume 100).
* Dents with the holes: DENTLESS 57 (was 90; best: lamp-sweep-flat43 volume 81, fc2 40 blocks),
  MINOR DENTS ONLY or better 160. b38 drops to minor (a swept wall cell).
* Record-eligible: 0. The blocker is repeater input (229 doors), then the hallway (143), isolation
  (34: sculk), footsteps (20), no seamless tier (18). Missing ONLY repeater input: b38 (38) and
  its family (b38r, b39, b40, b40r, e41), lamp-sweep-w41, m05-41 family and more: their control
  reads the lever block itself (observers watching it), so powering the support block does
  nothing. The 14 doors where repeater input works all fail the hallway (gravity g1/g4/g6/sf1/sf3
  family) or hear footsteps (sculk s1r, sculk2-s1).
* Repeater input fails for doors that read the lever block itself (observers watching it: fc2,
  b38's family) or its weak power into a non-support neighbor (props-dyn's finding).
* Every leader is QUART seamless (except tick-door 218: none); fc2 and mp8 43/44 have exactly ONE
  open-state blocker (fc2: O_m observer (2,-1,0) in the floor ring; mp8: the sticky piston (0,3,0)
  in the ceiling ring) with both closed sides clean.
* Open targets (no working door): SUPER SEAMLESS, any FLUSH or DELUXE (full or semi), every
  HIPSTER tier, FULL SYMMETRICAL, FULL/SUPER SYNC, CLEAN, NO STICKY PISTONS, TORCH AND DUST ONLY,
  REDSTONE BLOCK ONLY, ZOMBA-, ZOMBI-, record eligibility.
* Cheapest path to a first record-eligible door: a b38-family door whose lever-reading observer is
  replaced by something that sees the support block's power (b38 already passes hallway,
  contained, weatherproof, footsteps, 20 cycles, QUART seamless).
