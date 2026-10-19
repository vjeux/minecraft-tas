# Track mechspace: an exact, scalable decision procedure for the macro pulse model, then a wider mechanism space

Brief (coordinator, 2026-10-19 00:44Z): make families' mechanism search EXHAUSTIVE where it matters
(7x8 = 56 and 7x9 = 63 flat doors exist in the community), then widen it.

## Tool: `mctas door mechspace` (crates/piston-door/src/mechspace.rs)

The question per candidate (families' model exactly: `families::Mech::apply`, QC coupling on, pulse
model = a 2 gt pulse on every piston + levels on pistons extended at rest): is there an open state o
(FULL seamless, `Mech::is_open`) with rest ->* o ->* rest and the hole rule on that cycle?

* **Stubborn-set partial-order reduction** (Valmari; landmark form of Wehrle & Helmert): at a
  non-goal state only a set T(s) of events is expanded, with (D1) every T-event commutes with every
  sequence of non-T events and (L) no goal is reachable with non-T events only. Then EVERY reachable
  goal state stays reachable in the reduced graph (induction on path length), so the reduced open
  search finds every reachable open state and the reduced close search (goal = rest) is exact.
* T(s) from **exact footprints**: a re-implementation of `Mech::apply` that records every cell /
  power bit / QC bit it reads (writes = the diff). Events are grouped when one writes what another
  reads; each small group's local reach (its own events only) is explored exactly and its reads /
  writes over that reach collected; conflicting groups merge; big groups form the core U (only U's
  footprint at s matters, U is always expanded). The independent groups left outside have a product
  reach, so (L) is checked exactly per group.
* **Hole rule exact**: open states whose holes need cells kept air are re-searched in the restricted
  system (those cells air at every state of the cycle).
* **Lean stage** (`--cap-lean`): visited sets only (40-byte keys), plain search first (~300k states/s),
  then the reduced one; **A\*** (`--astar`, families' goal-directed search) can still FIND a solution
  when everything is capped. Found cycles are replayed with families' own `Mech::apply` (open state,
  exact rest, hole rule) before they count.
* Validation: `selftest` (tracked transitions vs `Mech::apply`: 305k applies, 0 mismatches; footprint
  independence: 2.7M non-conflicting pairs commute, 0 violations; compact-key round trips),
  `pretest` (pre-images: 699k forward edges, 0 missing), `validate` / `validate-list` (families'
  full-graph verdict beside ours).
* Commands: `selftest`, `pretest`, `validate`, `validate-list`, `decide --input list`, `gen` (families'
  generator with `--two/--carry/--vmid/--pistons`, every candidate decided), `refit` (every catalog
  mechanism re-solved in smaller boxes), `catlist`, `export` (catalog-style mech files + ideal plans
  replayed in the simulator), `diag`, `diag2`, `coreach`.

## Results

### Catalog mechanisms hidden in bigger boxes (refit, 01:27Z)
Re-solving every v3 catalog entry in the smaller boxes: 9 entries filed under bigger boxes FIT 7x8:
7x9-101, 7x9-102, 8x8-131, 135, 137, 138, 140, 8x9-157, 158 (also 8x8-128 / 136 / 139 / 141 fit
8x7; 8x10-159 / 160 fit 7x9). 8 of the 9 are in families' own 7x8 candidate list (exact canonical
match; 101 only as supersets), none is in unresolved/7x8.txt, and families' own `solve()` finds all 9
in 7x8 when asked directly: they were lost in the catalog pipeline (greedy minimize / describe under
the state cap), not by the generator bound and not by the cap.

### The corrected 7x8 list (list-7x8.tsv, mech/)
58 macro mechanisms = families' 49 + the 9 refits, each exported catalog-style (`mech/<id>.txt` +
`<id>-oc` / `<id>-sym` ideal plans, replayed in the simulator at 4 positions x 2 cycles):
31 have plans that WORK (sym and oc), among them the refits 8x8-131, 8x8-135, 8x9-158;
25 are rider shuffles with no ideal plan (a piston fires from a cell no input can reach), 2
(7x8-019 / 020, rider shuffles) were not re-solved within the caps.

### vmid check
vm1 (explore/vmid, 11 pistons, ML leaves up through a column-0 elevator) is re-found by `decide`
in 7x8 (15 + 9 events, 14 s) and flagged NEW (not in the v3 catalog).

### The wirable sub-model (`--no-door-bodies`, 03:20Z)
Families' "unresolved" blow-up is junk from pistons dragged INTO the 3x3 (rider shuffles): on
unresolved 7x8 #0 the full model has 1.84M reachable states and only 9,026 can return to rest. The
sub-model disables any transition that leaves a piston BODY inside the 3x3 (heads may enter). On
the corrected 58-list it finds exactly the 31 mechanisms whose ideal plans replay and proves the 27
plan-less rider shuffles impossible. State spaces collapse (#500 / #1200: > 20M -> 120 / 240).
In it, all v3-unresolved candidates are decided: 7x8 1861 (1834 no-open, 27 no-close, 0 found),
7x9 1945 (1902 / 41 / 2 over 20M states, 0 found), 7x7 1639 (all no-open). Main path: plain
enumeration of the reachable set R, then the states of R that can return to rest found BACKWARD
from rest (pre-images kept inside R); per hole mask a restricted re-search.

### New 7x8 mechanisms (all replay-checked in the simulator with ideal inputs)
* new-7x8-c2e.tsv (4, 11 pistons, vmid-any P <= 12 sweep): a column-2 elevator lifts MR out upward.
* new-7x8-t1a.tsv (16, 10 pistons, --two 1 sweep): BL pulled down by an extended-at-rest SU+(0,-2).
* `door connect` (wire64b): t1-001 / t1-002 wirable in their 7x8 placement, most others in 7x9.

### FULL FRAME (vjeux 03:36Z) and the single-U hint (04:04Z)
* `--full-frame`: a frame cell left air when open must stay air on the whole cycle; static stones are
  then added there and the augmented mechanism is re-decided and replayed (frame 12/12, stones
  static): `augment_frame`, `frame-export`. `--frame9`: all 9 door blocks in frame cells.
  The 4 c2e mechanisms pass with 3 stones.
* Single-U sweep (`--bottom-su --door-offset 2,2 --ext-pullers --frame9`): exactly one piston in
  the bottom row (sticky, facing up), extended-at-rest pullers for every 1-move exit, P <= 14,
  two = 0: 59,584 candidates; in the sub-model: 76 frame-12 cycles: the 009 family (and mirror) and
  a NEW layout (TL up by SD(0,4), TM up then right by the corner piston SW(3,3), static (-1,2)).
  Running: the same in the full model (bodies may cross the opening; slow), two = 1 (P <= 12).

### c78: the single-U 7x8 (04:58Z)
c79 (the community 7x9, reproduced on main) with BR leaving RIGHT instead of down gives a 7x8 whose
bottom row holds exactly one piston, SU(1,-2): SE(-2,0..2) SW(4,0..2) SE(-1,-1) SU(1,-2) SD(1,4)
SD(1,5) + static (0,3) (2,-1) (2,3), 10 pistons, open 11 / close 8, frame 12/12; C is pushed down
through TM by the elevator and taken by SU(1,-2), BM is pulled sideways into the floor corner by
the corner piston (-1,-1). c78-001..006 (level-held / extended variants) all replay (sym + oc)
in the simulator: c78.tsv, mech/c78-*. `door connect`: not wirable in 7x8 by connect's model
(c79's own mechanism is wirable by connect only in its real 7x9 placement).

### Piston-set enumeration (`door mechspace enum`, 05:15Z)
Pistons in the cells around the door (side columns, rows above / below, the 4 diagonal corners),
facing the door or along the frame (head cell or the next one in the frame or the door), sticky
(normal with MECHSPACE_FREE_NORMAL), retracted or extended at rest. Tractable structure: the 6
side pullers with at most `--two` missing and one extension state per side; `--bottom-su`; at most
`--carry` free pistons. Pruning: every door cell on a piston line, >= 9 frame cells receivable.
Acceptance: c79's piston set is generated in its 7x9 box (53.2M sets after pruning with free <= 5)
and solved. 7x8 single-U (offset 2,2, side <= 1 missing, free <= 4): 12.26M sets after pruning,
decided in the wirable sub-model on two boxes (MECHSPACE_SHARD), fewest pistons first.

## Log
* 01:03Z selftest 0 mismatches; 01:19Z pretest 0 missing; 01:27Z refit; 01:48Z corrected list.
* 03:20Z sub-model; 03:05Z c2e; 03:21Z t1a; 03:50Z full frame; 04:25Z single-U sweep.
* What did not work: the reduction alone on junk-heavy cores (pistons shoving each other: one big
  interacting group, no reduction); full backward search (the co-reachable set is > 20M even where
  the forward set is 1.8M: unreachable junk can also come back). Measured on families' unresolved
  7x8 #0: 1.84M reachable states, only 9,026 of them can return to rest.
