# e78 HANDOFF (e78k, session da291dc5, devvm47239) — 7x8 = 56 wool door, sculk category

Status: no verified 7x8 yet. Board partial: `minecraft-tas/board/partials/e78.md` (Space memory) has the
same facts with timings. All files below are on branch explore/q78 under `designs/piston-door-3x3/tracks/q78/e78k/`.

## What works (ideal-rs) and what is real
- `r1.txt` + `r1.ideal`: K is REAL. (2,-3) calibrated faces NORTH -> NORMAL piston (3,-3) facing east pops its
  head into (4,-3) -> observer (4,-2) facing down -> cross dust (4,-1) -> K = [12,14) on both edges (TILE rule:
  K must be [8..11,13) or [12,14)). Obsidian (-2,-1) ends K's push chain. The CLOSE works fully.
  Only flaw: on OPEN, (2,-3) has a 2nd cycle (t33..43, it hears the gate's t31 undo); its cooldown (t43) both
  lowers A' (B2) and retracts the relay, so K pushes BM into the empty F1 at t45 (A' lands t45, the gate's QC
  extension is t46). `r1x.ideal` (obsidian assist in F1 t44..46) WORKS.
  Ideal probes: relay held >= 1 gt past t43 WORKS; holding B' longer does not; a first-cycle-only relay pulse
  ([9..10, 11..13) or [9..10, 18..26)) WORKS. Nothing adjacent to the relay can hold it (only (2,-3) and the
  (3,-2) shield touch it; QC cells emit nothing).
- `r2.txt`: r1 + (0,-3) = observer facing west on (-1,-3) (B' pulses at (-1,-3)'s 3 transitions) + (2,-3) facing
  east. OPENS correctly by t36 (K's late pulses all blocked), then (-1,-3)'s 2nd cycle (it hears B2's B' at d=2)
  re-fires B' at t42 and A' at t46: cascade.
- `nb.txt` + `nb.ideal`: r2 with (-1,-3) = NOTE BLOCK (2 transitions). WORKS with only two ideal inputs: the
  lamp (-1,-2) powered [L, L+10) on both edges (L = 4..7) and the note powered [L+6..L+11, L+19) on OPEN ONLY
  (end exact: B2 lands A' at the gate's G3 start, A3 = gate QC). The close never tolerates the open window:
  B' must be EDGE-SELECTIVE. Blockers: only (-2,-3) can power the note and a sensor there hears BR's puller
  (t9) / A1 (t11) on both edges, no filter cell; a real lamp sensor at (-2,-2) re-activates at t29 (hears B2);
  (-1,-1) strongly powers the lamp, which conducts into the note block.

## Searched, all 0
- door fill e78wsT right cells k=5 (79M); shrink: e78wsT 5+1 cells k<=4 pistons (20M), 7 free cells k<=4 (60M),
  r1 left 5 cells k<=5 (81M), + top 2 k<=4 (73M), r2 6 cells k<=4 (37M); pockets3 `door fill --keep`: r1 7 cells
  <=4 (10M), 11 cells <=4 (94M), right cluster <=4 with repeaters/torches/comparators (54M), r2 left 9 <=4 (36M).
- cosearch r1 / r2 (left 10 cells, 40 min each): 0 finds (best 77/79).
- ideal-rs --vary around r1/r2/k1 (left sensors, lamp/note/bulb, amethyst, wool): 0.

## Mechanics learned (all from the source / redsim)
- Edge selectivity comes from occlusion by parked door blocks: on CLOSE, BM sits in F2 (until t12) and C in F1
  (until t8), so rays through F2/F1 are blocked; on OPEN the door cells (0..2, 0..1) block until t5.
  That is why (0,-3) hears BR's puller (t8) only on open (the r1 B1 stagger).
- Lamps change state with flag 2 (no neighbor updates, observers still see them); a sensor updates the
  neighbors of the block below it too; a 1-gt piston extension still pulls (type 1).
- Inherent: if K's relay and the gate share (2,-3)'s clock and A3 is the gate QC at (2,-3)'s cooldown, K's push
  (cool+2, block-event phase) precedes A's QC extension (cool+3).

## r1w (83ba8bee): (0,-3) wool + ideal open-only B' hold
r1 with (0,-3) = WOOL works with an ideal B' hold [10..19, 29) on open (tile); `--holds 40` also finds a
calibrated 10-tick hold starting at t18 on BOTH edges. (0,-3) MUST be wool: it shields the lamp sensor (-1,-3)
from B2 (B' at d=2) and from A' at (1,-2); stone/obsidian/amethyst there fail (the lamp sensor re-arms at t26,
hears B2, and its 2nd lamp cycle pushes C out of F1). So the hold has no cell: B' touches only (0,-3) and
(2,-3) (the relay/gate clock), and its QC cells are the A1 observer and the gate.

## Advice received (coordinator, c77)
- c77: a push fails only if the line has something immovable AT THAT MOMENT (obsidian, block entity, extended
  piston/head, moving block); pistons never retry. For t45: either K's update arrives 1 gt later (t46, when A's
  QC head is a moving block in F1) and nothing updates K again while powered, or K is powered without an update.
  A calibrated hold ends at activation + 10, so "ends at t29" = a calibrated sensor activating at t19; on close
  B' must be blocked at t19 (column moving / A' extended) and get no update before t29; on open free at t19 or
  nudged by an open-only update. c74 (explore/c77 tracks/c77/c074-64b-42.txt) and kb1g.ideal (stone e78s:
  B' = both-edge pulse pair s1 17..21, s2 s1+3..+6 <= 24, plus ONE open-only pulse at 26..50).
- coordinator: shrink's MOVING OBSERVER relay (archive/t7b-ob40-40b.txt: an observer fires when it LANDS, out
  of its face at the new cell, so the two edges can feed different cells); c88b's copper-bulb T flip-flop +
  comparator = a door-state LEVEL at the bottom (comparators need a support block, none on row -3).

## Next ideas
1. An edge-selective B'-hold for r1-style designs: a B' source active [19,29) on open only (B2 at t29 lands A'
   at G3 start; the 2nd (2,-3) cycle is then harmless as in nb).
2. Lamp sensor that does not re-arm into B2: a filter or occluder for the lamp sensor after t26.
3. A K relay that fires once per edge (no 2nd-cycle retraction), e.g. a block left in (4,-3) on open and
   brought back on close.
