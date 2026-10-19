# t78 handoff (2026-10-19 ~13:50Z)

## State
- No verified door. Best leads: r70 (c070x rotated into 7x8, works with ideal inputs + a real U observer; E's
  three pulses and U's exact offset are the open problem) and the a78/s78/hfh78 screens (all elevator tops
  satwire-UNSAT from one lever with the sensor bottom kept). c070x itself handed to c70 (tracks/t78/c070x/NOTES.md).
- Board: partials/r70.md and partials/a78.md (Space 2034923423883327, minecraft-tas/board/).
- Tools on explore/t78 (merge-ready, separate commit 0f8c4596): `door ideal-rs --holds-rank N` and
  `--offplane-dir DIR [--offplane-below Y]`. selftest 24/24 after merging main de6ecbce (c33f771b).

## Next steps
1. r70: get E's 3 taps without an echo: e.g. the calibrated sensor (3,4) wool-shielded from E's late moves, or
   E2 from a second source (QC through (1,5) also powers D: needs a gate), or make the echo taps blocked.
2. r70: U from a plain sensor out of the lever's range (8) hearing an early piston at the right distance;
   wool door (closed on open) hides the lever and the stack from the left side.
3. Ask satwire3 (sculk in SAT) to run r70 and the a78/s78 list once its sculk support lands (requested 12:35Z).

## Running processes
None (all satwire/pockets/annealer runs finished or cancelled).

## Files
tracks/t78/NOTES.md (sections 1-4), r70/, c070x/, a78/ (rankings, compacted plans, satwire logs), s78/, ideal/, mech/, cpt/, wip/.
