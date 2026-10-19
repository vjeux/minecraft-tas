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

## Log
* 01:03Z selftest 0 mismatches; 01:19Z pretest 0 missing; 01:27Z refit; 01:48Z corrected list.
* What did not work: the reduction alone on junk-heavy cores (pistons shoving each other: one big
  interacting group, no reduction); full backward search (the co-reachable set is > 20M even where
  the forward set is 1.8M: unreachable junk can also come back). Measured on families' unresolved
  7x8 #0: 1.84M reachable states, only 9,026 of them can return to rest.
