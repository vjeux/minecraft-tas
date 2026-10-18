# Track pistmodel: the complete, game-verified piston / moving-block model

Brief (vjeux via the coordinator, 2026-10-18 22:25Z): "The remaining ones are truly hard and will use
all the redstone capabilities. So we need proper models of everything we can use, not just simple
stuff." The judge runs the real game, but the mechanism search (`mechsynth.rs`, `families.rs`) uses a
simplified macro model (in-plane pistons, no helpers, limited pulses). This track builds the exact
tick-level model of pistons and moving blocks in Rust (`crates/redsim`, shared with track redmodel's
redstone model), fuzzes it against the real game, then upgrades the mechanism search to it.

1. [INVENTORY.md](INVENTORY.md): everything pistons do, read in the 26.3 source.
2. `crates/redsim`: one world / tick loop (redmodel: engine queues + redstone components; pistmodel:
   pistons, heads, moving pistons, structure resolver, falling blocks).
3. Differential fuzzing vs the bridge sandbox (every block state, every tick).
4. Mechanism search on the full model (helpers, pistons moved by pistons, spitting, 0-ticks).

## Log
* 22:33Z inventory written (INVENTORY.md); crate layout proposed to the coordinator.
