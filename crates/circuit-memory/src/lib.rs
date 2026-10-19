//! Contraption study 5, track memory (docs/circuits.md): `mctas circuit memory <args...>` dispatches
//! here. A stub from clockinfra: the track owns this crate and replaces `run` with its commands.

/// `mctas circuit memory <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas circuit memory: no commands yet (track memory owns crates/circuit-memory); got {args:?}")
}
