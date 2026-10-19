//! The official judge's redsim screen (crates/circuit: `mctas circuit sim`) on a design built in
//! memory: the census confirms every candidate with it before keeping it.

use mctas::design::Design;

/// (pass, metrics text, why) of `mctas circuit sim` on `d` (which carries its `circuit:` line).
pub fn judge_sim(d: Design) -> Option<(bool, String, String)> {
    let c = circuit::Circuit::from_design(d).ok()?;
    let rep = circuit::verify::sim(&c, &circuit::verify::Options::default()).ok()?;
    let out = rep.outcome?;
    Some((out.pass, out.metrics_text(), out.why))
}

/// A design from census blocks and ports.
pub fn design(name: &str, kind: &str, blocks: &[(crate::harness::Pos, redsim::Block)], inputs: &[(String, crate::harness::Pos)], output: crate::harness::Pos) -> Design {
    let mut d = Design { name: name.to_string(), ..Default::default() };
    for (p, b) in blocks {
        d.blocks.insert(*p, b.to_string());
    }
    for (n, p) in inputs {
        d.ports.insert(n.clone(), *p);
    }
    d.ports.insert("output".into(), output);
    d.meta.insert("circuit".into(), kind.to_string());
    d
}
