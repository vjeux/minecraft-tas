# Track redmodel: the exact, game-verified redstone model (`crates/redsim`)

Goal (vjeux): "The remaining ones are truly hard and will use all the redstone capabilities. So we
need proper models of everything we can use, not just simple stuff." Build ONE exact tick-level
model every search tool can share (pistons and moving blocks: track pistmodel, same crate), prove it
against the real game by differential fuzzing, and expose it to the search tools.

Where things are: the model `crates/redsim` (engine, redstone, containers, classes; pistmodel owns
`piston/` and `gravity.rs`), the differential runner `crates/redsim-fuzz` (`redsim-fuzz` binary and
`mctas door redsim-fuzz|redsim-case`), the judge screen `crates/piston-door/src/redsim_screen.rs`
(`mctas door redsim-verify`), fixtures `crates/redsim/tests/fixtures/` (sources in
`designs/redsim/cases/`), the per-component reference with source lines: docs/mechanics.md
section 12; what each tool models: docs/coverage.md. Inventory: [INVENTORY.md](INVENTORY.md).

## Results

* **0 mismatches on 100,000 random circuits** (7x4x7 boxes, density 0.5, `--palette all`: dust,
  repeaters, comparators, torches, levers, buttons, observers, lamps, bulbs, note blocks, targets,
  trapdoors, doors, gates, rails, droppers/dispensers/crafters, hoppers, slabs, leaves/logs, cake,
  composter, end portal frames, respawn anchors, skulls, plain blocks; 35% placed with updates, 40%
  with mid-run edits, random `use`s), every block state compared after every tick. Rates on the
  way: 54/50,000 (flags bug + hopper items), 0.249% (RailState missing), then 0.
* **0/20,000 container circuits** (`--gen containers`: droppers and hoppers with items,
  comparators reading them, floor levers) after item transfer went in; 10,000 random reruns 0.
* **Judge screen**: `door redsim-verify designs/piston-door-3x3/archive/*.txt --judge`: on the
  final model, 276 pass, 52 fail, 24 unsupported (sculk, entities, unwaxed copper), 0 verdict
  disagreements with the real judge on all 328 supported designs (first run: 263/263 at 2.2 s wall
  vs 28 s for the judge, 68 ms CPU per design vs ~3 s). cosearch built its search on it
  (2049/2049 agreement).
* **Final confirmation** (finished model with containers, scaffolding and pistmodel's 85d1ac7
  shapes): 0 mismatches in 100,000 random circuits (seeds 50000-50009, `--palette all`, 7x4x7)
  and 0/20,000 with pistons added (`all,piston:4`); `redsim-verify --judge` still 0 disagreements
  on the 328 supported archived designs.
* **Block equivalence classes** (`redsim::classes`): 70 plain states in 22 classes (11 ignoring
  note-block instruments), proven in the game by `redsim-fuzz classfuzz` (3000 circuits vs
  class-swapped variants: 0 differ). Sent to satwire, boxproof and cosearch.
* **Scaffolding** (vjeux's ask: a second 1 gt delay element after leaves): distance/bottom in a
  1 gt tick after any shape update or placement, break at 7, fall when placed at 7; 0/30,000
  scaffold-heavy circuits (no pistons), the gadget designs/gadgets/scaffold-delay as a fixture,
  two link-table rows (112/112 identical in the game: a tower passes edges up 1 gt per block and
  swallows 1 gt pulses). `behavior::collision_boxes` (every block's collision shape) added for
  pistmodel, which wired it into moving-block shapes and falling-entity collisions (85d1ac7, merged):
  falling scaffolds 0/10,000, scaffolds + pistons 0/10,000; only a FALLING scaffold moved by a
  piston still differs (13/10,000 circuits with both: entities pushed by moving pistons, pistmodel's
  open item).
* **Hopper T flip-flop** gadget (two hoppers facing each other, each locked by a torch under it,
  one item; a 2 gt pulse on both torch supports moves the item once: the receiver's 7 gt cooldown
  stops it coming back before the relock), identical to the game over 99 frames
  (`designs/redsim/cases/hopper-t-flip-flop.txt`, fixture).

## Model errors found by the fuzzer (each now a fixture or a test)

1. Shape-update flags: `Level.setBlock` passes `flags & -34` (drop UPDATE_NEIGHBORS and
   SUPPRESS_DROPS) to the shape updates it triggers; I had dropped UPDATE_CLIENTS. Changes made by a
   shape update notify no neighbors (fixture shape-update-no-neighbor-updates). pistmodel found it
   independently.
2. Cake survival needs `isSolid` below: it pops off a repeater (fixture
   cake-on-unsupported-repeater).
3. A wall skull's state has no instrument override: a note block under it plays the harp (fixture
   note-under-wall-skull).
4. `RailState` reshapes neighboring rails when a rail is placed (0.249% of cases before).
5. Hoppers next to popped blocks pick up item entities: excluded (flagged), hoppers kept for
   locking.
6. synth.rs (resynth/regrow's model) had slime as a non-conductor and leaves/honey as floors; it now
   takes plain blocks' conductor/floor from redsim (tests in synth.rs).
7. A HOPPER's `removeItem`/`setItem` do not call `setChanged` (HopperBlockEntity overrides them),
   so an eject + pull leaves no transient empty reading; droppers notify (fixture
   hopper-remove-no-setchanged).
8. Harness bug, not the model: the bridge's `set` shared one `MutableBlockPos` among the blocks of
   a request, and a piston's onPlace block event kept it (ran at the wrong cell). Fixed on main
   (4a4fd3e); the fuzzer places update-mode blocks one per request.

Mutation checks: making the lamp's off delay 6 gt gives 27% mismatches; reverting the flags fix
fails the fixture: the harness sees model errors.

## Integration

* satwire: the 8 semantic rules its encoding gets wrong or omits (tick order and priorities,
  dedup, comparator 2 gt recompute + compare-mode front updates, QC as BUD with the update table,
  observer transients, bulb rising edge, dust levels and order, torch burnout), the lamp rule (on in
  the powering update, off by a 4 gt tick from the first unpowered update), and redsim as its CEGAR
  screen (it now screens every SAT model with redsim before the judge).
* boxproof: the same rules + the screen API + the classes; cosearch: the screen API + the classes.
* `door topology` link types: the model now provides them: `redsim::links` (`redsim-fuzz links
  [--verify]`) measures each component's output windows for standard pulse trains in redsim and
  replays every (gadget, train) in the game (96/96 identical); the table is in docs/mechanics.md
  section 12 (observer busy window merges 1-2 gt pulses, observers on a level double the count,
  lamps merge gaps < 4 gt, comparators swallow 1 gt pulses, bulbs halve the count). topology itself
  works on macro events without tick times, so wiring the table into its verdict is its owner's
  next step (`links::response(gadget, toggles, ticks)` answers any train).

## Next ideas

* Wire `redsim::links` into `door topology`'s verdict (event times from the mechanism's ideal
  plan instead of change counts), and add more link gadgets (dropper/hopper flip-flops, leaves,
  latches made of a piston or a note block reset on the other edge).
* A container palette in cosearch/satwire (dropper/hopper flip-flops as memory links, 2-4 cells).
* Dispensers' block-placing behaviors (shulker boxes, blocks?) and droppers with several stacks are
  random or entity-based: keep them out unless a door needs them.
