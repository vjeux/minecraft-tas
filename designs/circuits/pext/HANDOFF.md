# pext HANDOFF (study 6, piston extenders) — track pext, branch explore/pext

## State
* Archive (`designs/circuits/pext/archive/`, `mctas circuit archive`, all robust 16/16 in the game):
  N=1 sticky (2 blocks); N=2 rider7 flat/3D (7, 9/9 gt), rider 8-block flat/3D, sf7-1/2/3
  slime-free (7, 7/12), sand6 gravity (6, 7/12), one-button versions of rider7 (stone, oak), sf7
  and sand6; N=3 rider-v1 (31, 24/24) and rider24 (24, 25/25).
* Mechanisms and the N=3 analysis: `designs/circuits/pext/NOTES.md`.
* Tools (crates/circuit-pext, `mctas circuit pext ...`): `sim` (redsim, support rule, extend =
  payload at the target with nothing moving, like the judge), `trace`, `verify` (real game, 4
  positions x 4 rotations), `grow` (exhaustive k-block additions in a region, `--near`, a base
  without `port input` takes an added lever/button as its input), `minimize` (removals,
  substitution+removal, place-in-empty+2 removals). Exploration-only `ideal:` header: scripted
  writes per phase (`ideal: both 5 set 4 1 1 minecraft:redstone_block; both 7 clear 4 1 1`).
* work/: scratch designs, bases (base-*.txt), ideal-r3*.txt (mechanism tests with ideal pulses).

## Next
* N=3 smaller: other wirings of the rider's inner train (C B C after each landing); a sand (gravity)
  N=3 (the top piston must stay extended: input-held at its final spot); flat versions.
* N=4+: the one-frame rider generalizes (inner train for the pistons on the frame = the pulse-toggle
  train T_{N-1}, e.g. D C D B C D), wiring grows; or self-triggered units (piston + slime + observer
  per stage, see NOTES).
* More N=2 families: flat slime-free, observer-free (repeaters/torches).
