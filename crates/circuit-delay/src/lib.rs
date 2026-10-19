//! Contraption study 3, track delay (docs/circuits.md): `mctas circuit delay <args...>` dispatches
//! here. A stub from clockinfra: the track owns this crate and replaces `run` with its commands.

/// `mctas circuit delay <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas circuit delay: no commands yet (track delay owns crates/circuit-delay); got {args:?}")
}
