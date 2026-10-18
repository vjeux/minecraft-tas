# 0-tick and 1-tick study (Minecraft 26.3, track `tick-study`)

What can happen inside ONE game tick (0 gt), what happens on ODD ticks (1 gt), which gadget shows
each effect, and what that means for the fastest possible 3x3 piston door. Everything here was
read in the decompiled 26.3 source (`mctas src show <Class>`, line numbers below are that
listing's) and every gadget was checked tick for tick against the UNMODIFIED official server
(`mctas door vanilla-gadget`, section 8). "gt" = game tick; "k" or "t" = ticks after the lever
flip (the flip happens between ticks, so the first tick after it is k1).

## TL;DR

* **Floors (solid doors): open >= 4 gt, close >= 1 gt** (section 6). The boards are at 10 / 7.
  The speed track's bound (7 / 6) is for one topology (the edge block above the center leaves
  through the center's exit cell); routing that edge block out through its corner instead lets the
  center start moving in tick 1, which is what makes 4 possible. On the close side, pistons
  carried into place by a 0-tick fire in the same tick: the center and the edge block come home
  with every move placed (`center-return`, `corner-return`). **The same machine opens in 4 and
  closes in 4 (`open-4-cycle`, ideal inputs, vanilla-identical)**: one carried-piston chain per
  tick (two chains started at one tick boundary interleave in the FIFO queue); same-tick relays
  would bring the close to 1-3.
* **The open floor is exactly the center**: it needs two moves; a pull (or drag, or plain push)
  lands 2 gt after it starts and can move again 1 gt later, so the second move is >= 3 gt after
  the first; a push can be finalized at once (0-tick), but its pusher then sits inside the
  opening and is stuck for 2 gt. Either way the center is out at k1 + 3 = **k4** at best. Gadget
  `center-exit` performs exactly that critical path (out of the center at k1, out of the opening
  at k4), and gadget **`open-4` clears a whole 3x3 opening at tick 4** (ideal inputs; the edge
  block above the center leaves through its corner); both match vanilla at every position.
* **0 gt** = inside one block-event phase. Block events (piston moves) run FIFO until the queue
  is empty, so any number of piston actions can chain in one tick as long as every link is
  triggered by something that happens inside the phase: a block a sticky piston finalTicks
  (`zero-tick`), a moved power source (`instant-off`), a conductor leaving a dust step
  (`instant-on`), a piston placed by a 0-tick (`zero-tick-onplace`). One block can even make two
  placed moves in one tick (`double-move`).
* **1 gt / odd ticks**: every redstone component delays by an even number of game ticks; odd
  ticks come from (1) pistons fired by the lever itself (k1), (2) moved blocks landing in the
  block-entity phase, AFTER that tick's block events, so pistons react one tick later
  (`landing-next-tick`), (3) 1 gt scheduled ticks (persistent leaves: `leaves-delay`), and are
  carried on by observers/repeaters (`odd-observer`, `odd-pulse`). Spitting turns a landing into
  a block-event-phase placement and saves exactly that tick (`spit-saves-tick`).

## 1. Anatomy of a tick (`ServerLevel.tick`, 358-460)

| order | phase | source | what runs there |
|---|---|---|---|
| between ticks | player actions | `LeverBlock.pull`, `ButtonBlock.press` | lever/button state + neighbor updates at once: dust re-powers, pistons QUEUE events, observers/repeaters/torches SCHEDULE ticks, lamps light, bulbs toggle |
| 1 | `handlingTick = true` | 360 | (used by `PistonBaseBlock.checkIfExtend`, see section 2) |
| 2 | game time + 1 | 387 `tickTime()` | |
| 3 | scheduled block ticks | 394 `blockTicks.tick` | repeaters, comparators, observers, torches, buttons, plates, lamps (off), leaves... by (time, priority, insertion) |
| 4 | fluid ticks | 396 | water 5 gt, lava 30 gt |
| 5 | block events | 409 `runBlockEvents` (1267-1292) | pistons (`triggerEvent`), note blocks, bells/chests; FIFO `removeFirst` until EMPTY, events added meanwhile run in the same phase; the queue is an `ObjectLinkedOpenHashSet` (215): an identical pending event is dropped |
| 6 | `handlingTick = false` | 412 | |
| 7 | entities | 431 | |
| 8 | block entities | 459 `tickBlockEntities` | `PistonMovingBlockEntity.tick` (progress, landing), sculk sensors, daylight detectors (every 20 gt), hoppers |

Neighbor updates go to W, E, D, U, N, S (`NeighborUpdater.UPDATE_ORDER`), so pistons powered by
one change queue their events in that order (`event-order`). Dust updates its neighbors and their
neighbors in `HashSet` order (position dependent; `DefaultRedstoneWireEvaluator`).

**Orientation-based update order:** 26.3 ships `ExperimentalRedstoneWireEvaluator` and passes an
`Orientation` through `CollectingNeighborUpdater` (149-155) and `PistonBaseBlock.moveBlocks` (336),
but `ExperimentalRedstoneUtils.initialOrientation` (9-22) returns null and
`RedstoneWireBlock.updatePowerStrength` (271-275) uses the default evaluator unless the world has
the `redstone_experiments` feature flag. Default worlds (the judge's, the official server's)
keep the fixed W,E,D,U,N,S order and the position-dependent dust order.

## 2. Pistons: the rules that set the timing

| rule | source |
|---|---|
| A power change only QUEUES an event (0 extend, 1 retract-and-pull, 2 retract-and-drop); the event re-checks power when it runs: an extend whose power is gone is dropped, a retract whose power is back just re-sets `extended` | `checkIfExtend` 93-114, `triggerEvent` 143-151 |
| An extend is only queued if the push resolves at that moment (no retry later: gadget `no-retry`) | 96-98 |
| Moved blocks become `moving_piston` at the destination at once; their old cells are air at once (head cell: the moving head) | `moveBlocks` 299-326 |
| A moving block's entity goes 0 -> 0.5 -> 1.0 in the block-entity phases of t and t+1 and is placed in the block-entity phase of t+2 | `PistonMovingBlockEntity.tick` 294-327 |
| Retract while extending: the head's moving entity is finalTick'd (to air: it is the source piston) | `triggerEvent` 164-166, `finalTick` 270-271 |
| Sticky only: if the block two ahead is a moving block this piston pushed (extending, same direction), it is finalTick'd: PLACED NOW, not pulled. Normal pistons only remove their head (the block keeps moving) | 180-190, 204-205 |
| Retract type 2 (drop) when the block two ahead is moving and (progress < 0.5 or this tick's entity tick already ran or the unpower happens in the scheduled/block-event phases); else type 1 (pull) | 101-110 |
| Type 1 with a plain block two ahead pulls it (also when the front cell was EMPTY: reach-through) | 192-202, `PistonStructureResolver` (retract starts at pos+2) |
| A pull is never finalized early: pulled and dragged blocks always land at t+2 | (finalTick needs `isExtending`) |
| The retracting base is a `moving_piston` (immovable, cannot act) until t+2; it lands with `setBlock(..,67)` + `neighborChanged`, so a powered piston re-extends at t+3 at the earliest | 168-179, `PistonMovingBlockEntity.tick` 313-314 |
| A piston placed by `finalTick` or a landing runs `onPlace` and queues an extend if powered (in the same phase when placed by a 0-tick) | `onPlace` 80-86 |
| A moving block's entity is also finalTick'd when its cell is overwritten (`preRemoveSideEffects`), which no piston action does (moving pistons are immovable) | `PistonMovingBlockEntity` 286-288 |
| Extended pistons, moving pistons, obsidian, block entities are immovable; 12 blocks max | `isPushable` 215-257 |

## 3. Catalog: what can happen in 0 gt (inside one block-event phase)

| effect | why (source) | gadget | vanilla |
|---|---|---|---|
| **0-tick piston**: a sticky piston extends and retracts in one phase; the block it pushed is PLACED in that tick | 180-190 (finalTick two ahead) | `zero-tick` | PASS |
| a 0-ticked NORMAL piston's block still lands at t+2 | 204-205 | `zero-tick-normal` | PASS |
| **reach-through**: a 0-ticked sticky piston with an empty front pulls the block two cells away | 101-110 (type 1: nothing it pushed is moving), retract resolver starts at pos+2 | `zero-tick-reach` | PASS |
| a piston placed by a 0-tick fires in the same phase | `onPlace` 80-86 | `zero-tick-onplace` | PASS |
| one block, two placed moves, one tick | finalTick's `setBlockAndUpdate` updates the next (armed) piston, which is 0-ticked in turn | `double-move` | PASS |
| power LOSS: a moved redstone block (or any moved conductor) cuts power at once; pistons on that line queue retracts that run in the same phase | `moveBlocks` (source becomes `moving_piston`) | `instant-off` | PASS |
| power GAIN without a moved source: a conductor pushed (or pulled) off a dust step connects the step at once | `RedstoneWireBlock` 150 (`canConnectUp = !above.isRedstoneConductor`) | `instant-on` | PASS |
| power GAIN from a placed source: a redstone block finalTick'd by a sticky retract powers its neighbors in the block-event phase (their events run in the same phase) | 180-190 + `finalTick` 276 | `zero-tick-onplace` (a piston instead of a redstone block), `spit-saves-tick` | PASS |
| an observer SEES a 0-tick (the cell changed twice inside the phase): one pulse 2 gt later | `ObserverBlock.updateShape` 67-68 | `zero-tick-observer` | PASS |
| a 0-tick POWER pulse (source placed and removed inside one phase): lamp lit 0..4 gt, repeater stretches it to its delay (on 2 gt later), comparator swallows it; a piston next to the source moves only if its event is queued before the one that removes the source (it is then 0-ticked itself) | `DiodeBlock.tick` 50-63, `ComparatorBlock` 145-146, `RedstoneLampBlock` 36 | `zero-tick-pulse` | PASS |
| event order = update order (W,E,D,U,N,S), FIFO; a piston whose front became a moving block meanwhile does nothing | `runBlockEvents` 1270-1271, `moveBlocks` returns false | `event-order` | PASS |
| a powered piston carried onto a powered spot by a LANDING fires next tick (landing is in the block-entity phase) | `onPlace` | `landing-fire` (existing) | PASS |
| a quasi-powered piston ignores its source until any neighbor update (BUD); arming one silently is how 0-tick partners wait | `getNeighborSignal` 116-136 | `qc-bud`, `silent-hold` (existing), every 0-tick gadget | PASS |

**Constraint for doors:** a BUD-armed (powered, not yet updated) piston cannot exist in the REST
state: the judge places a design with a `self` update of every block. Partners and armed pullers
must be armed after the flip: by the lever itself (it only updates its own neighbors and its
support's: a lever next to a piston's quasi cell arms it silently), by dust feeding a conductor
next to the quasi cell from 3+ cells away (dust updates reach 2), or by a redstone block placed
next to the quasi cell (its updates reach only its own neighbors).

## 4. What happens in 1 gt (odd ticks)

| source of odd timing | when | gadget | vanilla |
|---|---|---|---|
| pistons the lever powers directly move at k1 | lever between ticks, events in tick 1 | every lever gadget | PASS |
| a block pushed at t lands at t+2 in the block-entity phase, after that tick's block events: pistons it powers act at t+3 (k1 push -> k4), components it feeds schedule from t+2 (repeater delay 1: k5) | `ServerLevel.tick` 409 vs 459 | `landing-next-tick` | PASS |
| a spit block is placed in the block-event phase of the retract tick: pistons it powers act THAT tick (one tick earlier than a landing) | `checkIfExtend` 108 (`isHandlingTick`) | `spit-saves-tick`, `spit` (existing) | PASS |
| so a PUSHED block arrives after 0 gt (0-tick), 1 gt (its sticky pusher unpowered in the next tick's scheduled or block-event phase: spit at t+1), or 2 gt (spit at t+2 in the block-event phase, or landing in the block-entity phase); a PULLED or dragged block always after 2 gt (landing) | 101-110, 180-190 | `zero-tick`, `spit-saves-tick`, `landing-next-tick` | PASS |
| an observer watching a cell a k1 piston changes fires k3..k5; chains stay odd (k5, k7...) | observer 2 gt from the block-event phase | `odd-observer` | PASS |
| a repeater reading a redstone block a k1 piston moves falls at k1+2d; a note block turns that edge into a state change an observer sees | `DiodeBlock` 84-99 | `odd-pulse` (existing, speed-2) | PASS |
| comparator subtraction keeps the parity of its inputs: side input from a redstone block a k1 piston pushes (lands k3) -> output off at k5; pulled away at k1 -> output back at k3 | `ComparatorBlock` 145-146 | `comparator-odd` | PASS |
| **1 gt scheduled ticks**: persistent leaves recompute `distance` 1 gt after a neighbor change (also scaffolding, and 26.3's survival checks of sugar cane/cactus/bamboo/kelp/chorus, which break blocks); a piston at k1 -> leaves at k2 -> observer k4; a piston at k2 (even) -> leaves k3 (odd) | `LeavesBlock` 102, `ScaffoldingBlock` 83/103 | `leaves-delay` | PASS |
| water spreads every 5 gt, lava 30 (fluid ticks) | `fluidTicks.tick` 396 | - | - |
| sculk sensors wake floor(distance) gt after a vibration, in the block-entity phase (any parity, category sculk) | `SculkSensorBlock` 190/208 | `sculk-delay-line`, `sculk-hold` (existing) | PASS (sculk-delay-line, sculk-hold-cm) |

## 5. Components: timing table (all from the 26.3 source)

| component | input -> output | delay (gt) | phase / priority | notes | gadget |
|---|---|---|---|---|---|
| lever | click -> powered | 0 (between ticks) | - | strong power into its support, signal to adjacent components | all |
| stone / wood button | click -> on, then off | on 0, off +20 / +30 | scheduled | `ButtonBlock` 97 | `button-pulse` |
| pressure plate | entity -> on, off | +20 (weighted +10) re-check | scheduled | `BasePressurePlateBlock` 116 | - |
| redstone dust | any change | 0 | where the change happens | updates 2 cells deep, `HashSet` order | `instant-off`, `dust-reach` |
| repeater | input change | 2 x delay | scheduled; EXTREMELY_HIGH if it feeds another diode, VERY_HIGH when turning off, HIGH when turning on | a pulse shorter than the delay, even a 0-tick one, is stretched to it; locking is instant (side input) | `pulse-stretch`, `zero-tick-pulse`, `repeater-lock`, `early-window` |
| comparator | input change | 2 | scheduled; HIGH if it feeds a diode, else NORMAL | output recomputed in its tick: a change undone before then (a 0-tick pulse) is swallowed; subtraction keeps its inputs' parity | `zero-tick-pulse`, `comparator-odd` |
| redstone torch | input change | 2 | scheduled | burnout: 8 toggles in 60 gt -> off 160 gt (`RedstoneTorchBlock` 74, 87) | `torch-delay` |
| observer | shape update from its face | on +2, off +2 more | scheduled | ignores changes while powered; sees 0-ticks | `observer-delay`, `zero-tick-observer`, `odd-observer` |
| redstone lamp | power | on 0, off +4 | off scheduled | visible to observers | `lamp-fall-delay` |
| copper bulb (waxed) | rising edge | toggles 0 | in the update | comparator reads 15/0 | `bulb-toggle`, `bulb-pulse` |
| note block | rising edge | `powered` 0, sound = block event (same tick) | block-event phase | vibration for sculk | `odd-pulse`, `note-watch` |
| target block | projectile hit | power, off +8 (arrow) / +20 | scheduled | weak power; dust next to it bends into it | `target-tap` |
| lectern | page turn | 2 gt pulse | scheduled | `LecternBlock` 152 | - |
| daylight detector | time of day | updated every 20 gt (game time % 20 == 0) | block-entity phase | locked to absolute time; constant with `advance_time` off | - |
| bell | rising edge | rings at once, no redstone output | in the update | vibration | - |
| sculk sensor | vibration | active after floor(distance), 30 gt (calibrated 10), cooldown 10 | block-entity phase | category sculk | `sculk-hold`, `sculk-delay-line` |
| piston | power | powered between ticks (lever) or in the scheduled or block-event phase: acts in that tick's block-event phase (lever: tick 1); powered in the block-entity phase (a landing): next tick | block-event phase | moved blocks: section 2 | all |
| persistent leaves | neighbor change | 1 | scheduled | the only clean 1 gt element without a block that breaks | `leaves-delay` |

"1-tick" in the redstone community usually means one REDSTONE tick (2 gt, an observer pulse);
this study uses game ticks throughout.

No gadget for: lectern (its pulse needs a page turn, a menu click), daylight detector (follows the
time of day, frozen in the judge's worlds; a right-click inverts it at once), bell (no redstone
output), pressure plates and target blocks (need entities or projectiles). Their rows above are
read from the source only.

### Pulse generators

* **0-tick pulse on a piston** (extend and retract in one phase): power it, and have its own move
  update a silently armed partner that moves its power source away (`zero-tick`; the speed
  doors' "F 0-ticks" is the same pattern with landed redstone blocks). Variants: `-normal`,
  `-reach`, `-onplace`, `double-move`.
* **0 gt power edges**: `instant-off` (moved source), `instant-on` (conductor off a dust step);
  **0 gt power pulse**: `zero-tick-pulse` (a redstone block placed and shoved away in one phase).
* **2 gt pulse**: an observer (`dual-edge`, `spit`); **20/30 gt**: buttons; **any 2d**: repeater.
* **Odd pulses**: k3..k5 from an observer on a k1 piston (`odd-observer`); k5..k7 / k7..k9 from a
  repeater reading a k1-moved redstone block + note block + observer (`odd-pulse`); parity flip by
  persistent leaves (`leaves-delay`).

## 6. Floors for the 3x3 door

Cells: opening x, y in {0,1,2} in the plane z = 0; the walkway is the same x, y for |z| <= 3.
C = center (1,1); T = an edge cell next to it (say (1,2)); E = the cell outside T ((1,3)). A
door is open when no cell of the opening or walkway blocks (moving blocks block) from that tick
on; closed when the 9 cells hold their rest blocks (placed) from that tick on.

### Open >= 4 (proof)

1. Nothing moves before tick 1: the flip only queues events, they run in tick 1's block-event
   phase (section 1).
2. C is 2 cells from the outside of the 3x3 x 7 box in every direction (4 along z), and a piston
   moves a block 1 cell per move: C needs 2 moves, the second one taking it out.
3. A cell a block moves out of is air at once (section 2), so "open" is the tick in which C's
   last move STARTS (if everything else is out by then).
4. C can only move again once it is a placed block. After a pull, a drag or a plain push it is
   placed in the block-entity phase of t+2, so the next move is at t+3. The only early placement
   is `finalTick` by a sticky piston retracting while C is the block two ahead of it, i.e. C was
   the FIRST block of that piston's push: the piston's base is adjacent to C's start cell, and
   all six cells next to the center are in the opening or the walkway. That base is a
   `moving_piston` for 2 gt after the retract (t' >= t) and can be moved out at t'+3 at the
   earliest; the door is not open before.
5. Either way open >= t + 3 with t >= 1: **open >= 4**.

### An open-4 schedule (every step is a gadget)

The edge block T must leave T's cell before C's first move, and NOT through E (C's puller sits
there): it goes sideways through its corner, which costs it 2 moves too (k1 into the corner, k4
out), in parallel with C.

| tick | moves | gadget |
|---|---|---|
| k1 | the lever's piston shoves the redstone block feeding the pullers' dust line: the 6 side/bottom pullers (extended at rest, heads touching their blocks) retract in the same phase: ML, BL, MR, BR, TR, B out; TL pulled up into (0,3) | `instant-off` |
| k1 | TL's departure updates P_T at (-1,2) (armed after the flip): it extends into TL's empty cell and is 0-ticked by its partner X_T (in P_T's quasi cell): reach-through, T pulled into (0,2) (lands k3) | `zero-tick-reach` |
| k1 | T's departure updates A' at E (quasi-powered through F's cell): it extends into T's cell, its partner X_A shoves that source up and updates it: reach-through, C pulled into T's cell (lands k3); the shoved redstone block lands next to F's quasi cell at k3 and arms F silently | `center-exit` (tick 1) |
| k4 | an observer chain off the lever (2 observers: k4) unpowers the two aside pullers: A' pulled out of E, P_T pulled out of (-1,2) | `observer-delay`, `center-exit` (Y) |
| k4 | E's emptying updates F (above E): F extends into E, its partner X_F shoves its quasi source, F retracts: reach-through, C pulled into E: **C out** | `center-exit` (tick 4) |
| k4 | the same at (-2,2): G_T reach-pulls T out of TL's cell into (-1,2): **T out, open at 4** | `center-exit` (tick 4) |

**Built: gadget `open-4`** (ideal inputs: the scripted redstone blocks stand for the lever flip at
k0 and an observer chain at k4): 9 stone door blocks, 7 plain pullers, the 4 reach-through
pullers with their partners and the 2 aside pullers. The opening is full at k3 (T and C sit in
the top edge cells) and clear from k4 on; vanilla: section 8. What remains for a real door: wire
the inputs (lever, instant-off line, arming paths after the flip, the k4 observer chain); the
close of the same machine is built with ideal inputs too (`open-4-cycle`, next section).

The speed track's topology (T pulled into E at k1, lands k3; a pusher shoves [A', T] at k4 so a
puller takes E; C into T's cell at k4, out at k7) has floor 7; the best wired door opens at 10.

### Close >= 1, and what each schedule costs

Close is measured on placed blocks, so a 0-tick placement counts in the tick it happens. Trivially
close >= 1 (nothing moves before tick 1). C needs 2 moves in (E -> T's cell -> center) and T must
enter its cell after C went through:

* **close 1-3 (with same-tick relays)**: C pushed E -> T's cell by a 0-ticked
  piston above E (C placed: `zero-tick`), a piston carried into E by a 0-tick fires in the same
  phase (`zero-tick-onplace`) and 0-ticks C into the center; **gadget `center-return` does this
  for the center: C placed in the edge cell at k1 and in the center at k2** (the carried piston is
  ordered after E emptied by the tick boundary; a relay would put it in k1, `double-move` shows
  two placed moves of one block inside one tick). T comes back through its corner: pushed into
  the corner from the side cell R by a 0-tick, a piston carried into R fires and 0-ticks T into
  its cell, the corner block enters from the other side cell U. The carried pistons must be
  ordered AFTER the cell they enter is emptied, which a 2-cell-away pusher cannot see directly: a
  relay (a piston next to the cell that pushes a conductor off a dust step feeding the pusher:
  `instant-on`) does it in the same tick, the tick boundary in the next. **Gadget
  `corner-return` does T's part: T placed in the corner at k1 and in its own cell at k2 (pushed by
  a piston carried into R at k2), the corner block placed at k3.** Each piece works with
  tick-boundary ordering on its own; together, C's and T's chains must not share a tick boundary
  (see the caveat), so tick boundaries alone give C k2, T k3, corner k4 (close 4, built below), and
  a same-tick relay from the end of C's chain to the start of T's gives close 3 (2 if the corner
  follows by relay too, 1 with relays everywhere and 0-ticks for the plain pushes). The 6 other
  blocks only need plain pushes at k1 (landing k3).
  **Caveat found when assembling (next bullet): two carried-piston chains started by the same tick
  boundary interleave in the FIFO block-event queue, so C's and T's chains need a same-tick
  relay to share k2; with tick boundaries only, the assembled machine closes in 4.**
* **close 6**: the speed track's: C pushed into T's cell at k1 (spit), T pushed into E, F
  re-extends at k4 pushing [T, C] together; C is the second block, not finalized: lands k6 (the
  boards: 7).
* **close of the open-4 machine: built, close 4** (`open-4-cycle.drive`: the same machine opens
  in 4 and closes in 4, ideal inputs). Its open state IS the close layout: C in E under F, A'
  parked beside E on Y_A, T in the side cell (-1,2) in front of G_T, P_T parked beside it on Y_P,
  TL above its cell under TL's puller. F, Y_A + A', G_T, Y_P + P_T and TL's puller play
  `center-return`'s G, Q2 + P2 and `corner-return`'s A, Q3 + P3, B: k1 F 0-ticks C into T's cell
  and G_T 0-ticks T into TL's cell, the 6 plain pullers push their blocks home (land k3); k2 Y_A
  0-ticks A' back into E, A' fires as it is placed and 0-ticks C into the center; k3 Y_P 0-ticks
  P_T back, P_T fires and 0-ticks T into its cell; k4 TL's puller 0-ticks TL home. Started in the
  same tick, the A' and P_T chains interleave (FIFO: P_T fired while A''s head was still in T's
  cell, and a failed push never retries), hence one chain per tick; a relay from the end of one
  chain to the start of the next would give close 2 (k2: C then T, TL at k3 or k2).

Open + close >= 5; **the open-4 machine does open 4 + close 4 = 8 (built, `open-4-cycle`)**
against the boards' 10 + 7; wiring it to a lever (instant-off line, arming after the flip, an
observer chain for k4 and for each close tick) is the next step (tick-door track).

### What a lever-driven open-4 must provide (the script's inputs)

| when | scripted input (open-4-cycle.drive) | role | what can provide it |
|---|---|---|---|
| open k0 | air at the 7 pullers' redstone blocks, TL's first | the 7 plain pullers retract at k1; TL's puller must run before B's (F's update while B still blocks it) | one lever-fired piston shoving the redstone block of a dust net feeding them (`instant-off`); the dust order is position dependent, so feed TL's puller separately and first (update order W,E,D,U,N,S) |
| open k0 | redstone blocks next to the quasi cells of X_T and X_A, and R_T | arm the k1 partners and P_T silently | the lever itself next to a quasi cell (it updates only its own and its support's neighbors), or dust -> conductor -> quasi cell from 3+ cells away |
| open k3->k4 | air at Y_A's and Y_P's redstone blocks | the aside pullers retract at k4 | an observer pair off the lever (k4) firing a piston that shoves their redstone block (`observer-delay` + `instant-off`) |
| open k3->k4 | arm G_T (R_G + X_G) and X_F | k4 partners | the same k4 pulse, or R_A's own landing (F is armed that way already) |
| open, idle | disarm the 4 partners | ready for the close | any time while open (each removal updates the partner's head) |
| close k0 | the 6 plain pullers' redstone blocks back | plain pushes at k1 | power ON at k1 on the lever-off edge: an instant-on (a conductor pulled off a dust step, `instant-on`); a redstone block merely pulled back lands at k3 and the pushes would land at k6 |
| close k0 | F and G_T 0-ticks (R_F2 / R_G2 ordered before X_F2 / X_G2) | C and T into the top-row cells, placed at k1 | the lever-off edge reaching the pistons before their partners (update order), partners armed after the open |
| close k1->k2 | R_A3 + X_A arming, Y_A 0-tick | A' carried back into E, fires, C home at k2 | an observer pulse at k2 off the lever-off edge |
| close k2->k3 | R_T4 + X_T arming, Y_P 0-tick | P_T carried back, fires, T home at k3 | a k2 observer + 1 more link (odd: k3 comes from a k1 piston's observer, `odd-observer`) |
| close k3->k4 | R_TL3, X_TL arming, a poke next to TL's puller | TL home at k4 | an observer pair at k4 |

Observers fire on both lever edges, so every observer-driven input of the open also fires on the
close (and vice versa): the wired door must make those runs no-ops (`blocked-push`, `order-flip`
patterns) or route the two edges apart (a copper bulb toggles once per rising edge: `bulb-pulse`).

## 7. Notes for door builders

* Pistons react to a landing one tick late (landing = block-entity phase): spit (2 gt pulse) or
  0-tick to place blocks in the block-event phase instead.
* A reach-through puller (sticky piston, empty front, 0-ticked) moves a block two cells away
  without ever having its head next to it at rest: the cheapest way to pull C or T out of a
  cell that was full at rest.
* Arm 0-tick partners after the flip (section 3's constraint), and give each one a power source
  it can shove away.
* Use one update to order several pistons (W,E,D,U,N,S), or chain them through their own moves:
  every piston action updates its base's and front cell's neighbors.
* Two chains of events started together interleave in the FIFO queue (each event appends its
  follow-ups at the end): if chain B needs chain A's result, start B from A's last action (a
  relay) or one tick later. A piston whose push fails when its event runs never retries.

## 8. Vanilla confirmation

`mctas door vanilla-gadget designs/gadgets/<g>.drive` (crates/piston-door/src/vanilla_gadget.rs):
places the gadget's rest state in the official 26.3 server (`setblock ... strict`), replays the
script (`use` by a bot player, `setblock` = the game's own command in both worlds) and compares
the watch box after every action and every tick until the simulator is idle, at the judge's 4
positions. Logs: `designs/piston-door-3x3/tracks/tick-study/logs/`.

| gadget | result |
|---|---|
| zero-tick, zero-tick-normal, zero-tick-reach, zero-tick-onplace, zero-tick-observer, zero-tick-pulse | PASS, identical at 4 positions |
| instant-off, instant-on | PASS, identical at 4 positions |
| landing-next-tick, spit-saves-tick, leaves-delay, odd-observer, comparator-odd | PASS, identical at 4 positions |
| center-exit, double-move | PASS, identical at 4 positions |
| open-4, open-4-cycle, center-return, corner-return | PASS, identical at 4 positions |
| event-order, button-pulse, torch-delay | PASS, identical at 4 positions |

The rest of the library was run through the same tool (logs/vanilla-gadget-library.log): the 25
older gadgets whose scripts only `use` levers (bulb-pulse, bulb-toggle, dual-edge, dust-below,
early-window, flag-observer, head-qc, hold-2rep, lamp-fall-delay, landing-fire, lever-observer,
note-watch, observer-delay, odd-pulse, order-flip, pulse-bridge, pulse-stretch, pulse-toggle,
pump, repeater-lock, sculk-delay-line, sculk-hold-cm, slime-rider, spit, target-tap) are all PASS,
identical at 4 positions. The other 15 use the simulator's own `set`/`clear` placement (no vanilla
twin); rewriting their inputs with `setblock` would let the tool check them too.
