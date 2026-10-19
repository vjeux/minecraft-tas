# t78 handoff (2026-10-19 14:58Z, slot reassigned by the coordinator)

## State
- No verified door. Best lead: hs23-0008 (7x8, mechspace's "door 3 rows up" held-first batch), tracks/t78/hs23/.
  - h8L.txt/.ideal: hs23-0008 with the real lever (-3,2) on (-2,2)'s west face (holds (-2,2) direct, (-2,1) by QC). WORKS.
  - h8Q.txt/.ideal: + a pockets filling (sculk, k4) making A (bottom elevator, at (1,-1) and at home) and the BR puller
    real (calibrated sensor (4,-3), observer (3,-2) east, normal piston (4,-2) up, observer (3,-1) east). WORKS
    (open 38 / close 20). The 6 remaining inputs are ideal and OFF-PLANE.
  - Separately drivable (pockets, sculk palette, --gaps): SU(0,-2) both edges (bottom-left, 2 blocks), SE(-1,3) both
    edges (top-left, 3 blocks), B's two open pulses (bottom-right, 1 block). Open: TR+MR (exactly t30 open / t18
    close: one source at (4,3) = TR direct + MR by QC) and SD(1,4) (o30, c6, c18) in the 5-cell top-right pocket
    (local k5 run stopped at 293M of 345M fillings without a driver yet: hs23/pk-h8Q-k5-partial.log).
  - In flight on OTHER agents' boxes (results go to whoever owns hs23 next; ask them): pockets3 (1d42739b) re-running
    h8Q (k5, 3M combos) + the 24-plan hs23 batch at k3 with sculk on devvm49255; satwire3 (55a1288a) running sculk SAT
    (csculk+wool, lever ON, --hold auto) on the 24 hs23 plans.
- Other leads: r70 (c070x rotated into 7x8, works with ideal inputs + a real U observer; E's 3 pulses/echo and the
  U/M offset are open: NOTES sections 4-5); a78/s78/hfh78 screens (elevator tops satwire-UNSAT from one lever).
  c070x itself is c70's (tracks/t78/c070x/NOTES.md has my findings).
- Tools (merge-ready, own commit 0f8c4596): `door ideal-rs --holds-rank N`, `--offplane-dir DIR [--offplane-below Y]`.
  Branch merged with main de6ecbce (c33f771b), selftest 24/24.

## Exact next steps
1. Collect pockets3's h8Q result (pocket 4 = TR+MR+SD(1,4)) and the combination step; if pocket 4 has no joint
   filling, free (0,4)/(2,4) by driving SD(1,4) from the top-left pocket (it drove SD(1,4)'s c6 with 3 blocks) and give
   TR+MR the top-right alone, or try the other 23 hs23 plans (right-held ones put the lever near TR/MR).
2. Combine the separately found fillings (SU(0,-2), SE(-1,3), B) with h8Q in redsim (`door ideal-rs`), check sensor
   cross-talk (each new sensor also hears the others' pistons; wool-shield or retime), then `door verify --extra-offsets 28`.
3. Ask satwire3 for its hs23 sculk-SAT results.

## Running processes
None of mine (all local pockets/satwire runs stopped).

## Files
tracks/t78/NOTES.md (sections 1-6), hs23/, r70/, c070x/, a78/, s78/, ideal/, mech/, cpt/, wip/.
