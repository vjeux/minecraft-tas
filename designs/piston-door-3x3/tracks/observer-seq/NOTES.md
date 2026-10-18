# observer-seq track: event-driven sequencing

## Results (all verified with `door verify --extra-offsets 28`, archived in archive/)

| blocks | file | category | what changed |
|---|---|---|---|
| 58 | s6.txt = BEST.txt | solid, slime-free | stack feeds behind the stacks; lever under its block; right feed off the late-A repeater |
| 62 | s5.txt | solid, slime-free | P_T from the left lever chain; right stack from a chain on the late-A repeater |
| 65 | s4.txt | solid, slime-free | observer chains instead of routed dust for both stacks |
| 84 | s2.txt | solid, slime-free | P_T from an observer chain on Otl |
| 89 | s1.txt | solid, slime-free | first slime-free door (first entry on that board) |

Built by `crates/piston-door/src/observer_seq.rs` (`mctas door gen obs-s6`, ...).

## s6, the mechanism (58 = 19 mechanism + 39 control)
- Every piston is RETRACTED at rest and driven by one 2gt observer pulse: it PULLS when its head
  extends into air and PUSHES + SPITS when it pushes a block (the pulse ends in the scheduled-
  tick phase while the pushed block is still a moving_piston -> retract type 2).
- Side columns: two stacks of 3 sticky pistons ((-2,0..2,0) facing east, (4,0..2,0) facing
  west). Each stack is fired by ONE cross dust behind it at (x,2,-1) on (x,1,-1): the dust points
  into the top piston, its floor powers the middle one and quasi-powers the bottom one.
- Left stack: the lever hangs under its block K (-2,5,-1); the observer (-2,3,-1) under the lever
  pulses the left feed on both edges.
- A on both edges: Otl (0,3,0) watches the left column's top stone and pulses (0,4,0) -> A
  (opening: A pulls T up; closing: A pushes C one down, spitting it).
- P_BM: Obl (0,-1,0) watches the left column's bottom stone -> (0,-2,0) -> P_BM.
- P_T (1,3,1) facing north spits T forward into the park (1,3,-1) / pulls it back; fed by the
  chain (-2,3,0) -> (-2,3,1) -> (-1,3,1) -> (0,3,1) from the left feed observer (+8gt).
- Park observer (1,3,-2) fires once per direction (T parks / unparks): it feeds R_B (1,5,-1)
  (8gt B window; locked by RL (0,5,-1) from the torch on K while the lever is off) and, through
  the 1-tick late-A repeater (3,5,-1), the X net: in the closing that late A pushes T and C home
  together (2-block push); in the opening it is harmless.
- O (2,5,0) watches B (two pulses per held window) -> D1 (3,5,0) -> X (2,4,0) on (2,3,0): A pulls C
  up twice (v2's UP dance).
- Right stack: (4,5,-1) watches the late-A repeater (switches once per direction) -> (4,4,-1) ->
  strongly powers (4,3,-1) above the right feed dust.
Opening = {sides, M, A1}, P_T, window (B1+A2, B2+A3). Closing = {sides, M, A'}, P_T', late A.

## Findings
1. All six (or ten, slime-free) pistons can be retracted-at-rest toggles, one observer pulse
   per action (wip/toggle-mech*: validated with ideal pulses). No torches, no held states.
2. The closing needs no B: A pushes C down one, T comes back, one A pulse pushes T+C together.
3. e1 (solid, with slime): P_L at (0,3,0) shares (0,4,0) with A; the right structure's slime
   (2,3,0) drags T (no P_T); P_BM is a toggle fired by the bottom observer. Opens/closes with
   ideal pulses (wip/e1m-drive.txt); not wired (estimated 46-50, the solid record is 47).
4. Observer chains are the cheapest transport for pulses: 1 block and 2gt per cell, no floors,
   pulse width kept (a repeater with delay >= 2 stretches a 2gt pulse and kills push+spit).
5. Once-per-direction sources: an observer on a moving STONE (door block), on the lever, on a
   dust/repeater that pulses once, or on a HELD piston base. An observer on a TOGGLED piston base
   double-pulses (ext, moving, placed 4gt later after the observer turned off).
6. A piston blocked by a moving block re-checks when the block lands (the landing updates it):
   pulse-timed pushes can rely on it (used by the closing late A).
7. Repeater as R_B at (1,5,-1) quasi-powers A through B's cell: if B is ever blocked when R_B
   fires, A extends and wastes the window (s3 failed this way).

## What failed and why
- Locking R_B while O watches R_B: the lock changes R_B's state, O fires. O watches B instead.
- X dust fed from the north by a repeater: it connects and stops pointing into A.
- Isolated dust placed as plain `redstone_wire` canonicalizes to a DOT (powers nothing
  sideways): write the cross state explicitly.
- Late A from an observer on P_T's base (s3, -3 blocks): its pulses end 2gt too late, B is
  blocked by the moving A and A extends from R_B's quasi-power (finding 7).
- Inner side pushers for slime doors take the cells the X/window and lock need; the lever must
  sit at the lock's torch, which fights the stacks' feeds (hence the slime-free layout).

## Next ideas
- Combine logic-min's closing (window in both directions, B-off pulse pushes T in, steady P_T)
  with these observer-chain feeds: it removes the park line, late A and lock (~14 blocks).
- Wire e1 for the solid board with a structure gate (a block glued to the right structure in
  the window feed) instead of a lever lock.

## Tools
- `door drive`: `box x0 y0 z0 x1 y1 z1`, `changes N` (per-tick 3D block change log), `mark`.
