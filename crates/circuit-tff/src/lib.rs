//! Contraption study 2, track tff (docs/circuits.md): `mctas circuit tff <args...>` dispatches here.
//! A stub from clockinfra: the track owns this crate and replaces `run` with its commands.

/// `mctas circuit tff <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas circuit tff: no commands yet (track tff owns crates/circuit-tff); got {args:?}")
}
