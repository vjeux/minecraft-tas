# 1x2 piston door, track pext (2026-10-21)

The judge (`door-size: WxH`, crates/piston-door/src/lib.rs `door_dims`; docs/rules.md) and the
boards: [../../WR.md](../../WR.md).

## Verified (32 positions, rotation-safe, archived)
- `stacked.txt` (archive/stacked-5b.txt): 5 blocks, volume 4 (2x2x1), open 1 / close 3, solid,
  slime-free, flat, seamless SUPER, frame 2/6.
- `stacked-frame.txt` (archive/stacked-frame-9b.txt): the same with stone on the other three sides
  of the opening: 9 blocks, volume 4, seamless SUPER, frame 6/6.

## Tried
- The lever on a block behind the top piston (the selftest's classic): works (6 blocks), but the
  block is circuitry: volume 6 (3x2x1). The lever on the piston's back needs no block.
- Pistons above and below the opening (1x6): one column cannot carry the lever's power between the
  two pistons.
