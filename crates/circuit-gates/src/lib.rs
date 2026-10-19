//! Contraption study 4, track gates (docs/circuits.md): `mctas circuit gates <args...>` dispatches
//! here. A stub from clockinfra: the track owns this crate and replaces `run` with its commands.

/// `mctas circuit gates <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas circuit gates: no commands yet (track gates owns crates/circuit-gates); got {args:?}")
}
