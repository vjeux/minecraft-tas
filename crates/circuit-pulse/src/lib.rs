//! Contraption study 1, track pulse (docs/circuits.md): `mctas circuit pulse <args...>` dispatches
//! here. A stub from clockinfra: the track owns this crate and replaces `run` with its commands.

/// `mctas circuit pulse <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas circuit pulse: no commands yet (track pulse owns crates/circuit-pulse); got {args:?}")
}
