//! Contraption study 6, track pext (docs/circuits.md): `mctas circuit pext <args...>` dispatches
//! here. A stub from clockinfra: the track owns this crate and replaces `run` with its commands.

/// `mctas circuit pext <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas circuit pext: no commands yet (track pext owns crates/circuit-pext); got {args:?}")
}
