//! `mctas circuit fly prep`: get census finds ready for the archive: a readable name
//! (`fly-<class>-<blocks>b-<speed>gt-<type hash>`), a caption (the first comment block, which the
//! archive keeps) saying what moves what, from the structure and the travel direction, and the
//! redsim facts (speed, first move, rotations) as a second comment line.

use crate::machine::Machine;
use crate::report::FlyReport;
use crate::sim::{self, SimOptions};
use clock::Pos;
use std::collections::BTreeMap;

fn dir_of(facing: &str) -> Pos {
    match facing {
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        "south" => [0, 0, 1],
        "north" => [0, 0, -1],
        _ => [0, 0, 0],
    }
}

fn prop<'a>(state: &'a str, key: &str) -> Option<&'a str> {
    let inner = state.split_once('[')?.1.trim_end_matches(']');
    inner.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))
}

/// A facing relative to the travel direction.
fn relative(f: Pos, travel: Pos) -> &'static str {
    let dot = f[0] * travel[0] + f[1] * travel[1] + f[2] * travel[2];
    if dot > 0 {
        "forward"
    } else if dot < 0 {
        "back"
    } else if f[1] > 0 {
        "up"
    } else if f[1] < 0 {
        "down"
    } else {
        "sideways"
    }
}

/// The caption: the pistons and observers by their facing relative to the travel, the sticky
/// blocks, the kick.
pub fn caption(m: &Machine, r: &FlyReport) -> String {
    let travel = r.direction().or(m.travel).unwrap_or([1, 0, 0]);
    let mut pistons: Vec<String> = Vec::new();
    let mut observers: Vec<String> = Vec::new();
    let mut others: BTreeMap<String, usize> = BTreeMap::new();
    for s in m.design().blocks.values() {
        let id = s.split('[').next().unwrap_or(s).trim_start_matches("minecraft:");
        let f = prop(s, "facing").map(dir_of).unwrap_or([0, 0, 0]);
        match id {
            "piston" => pistons.push(format!("piston facing {}", relative(f, travel))),
            "sticky_piston" => pistons.push(format!("sticky piston facing {}", relative(f, travel))),
            "observer" => observers.push(relative(f, travel).to_string()),
            "piston_head" => {}
            other => *others.entry(other.replace("_block", "")).or_default() += 1,
        }
    }
    pistons.sort();
    observers.sort();
    let others: Vec<String> = others.into_iter().map(|(k, n)| if n == 1 { k } else { format!("{n} {k}") }).collect();
    let dir = if travel[1] > 0 {
        "flies up"
    } else if travel[1] < 0 {
        "flies down"
    } else {
        "flies horizontally"
    };
    format!(
        "{dir}: {}; {} watching {}; {}",
        pistons.join(" + "),
        if observers.len() == 1 { "1 observer" } else { "observers" },
        observers.join(" + "),
        others.join(" + ")
    )
}

/// The prepared design text and its name.
pub fn prep(m: &Machine, text: &str) -> (String, String) {
    let r = sim::simulate(m, &SimOptions { distance: 16, ..Default::default() });
    let class = crate::class_of(&r);
    let speed = r.speed_text().replace(" gt/block", "").replace('/', "_");
    let key = {
        let (n, _) = crate::ident::normalized(m);
        let k = clock::canon::type_key(n.design().blocks.iter());
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        k.hash(&mut h);
        h.finish() & 0xffffff
    };
    let name = format!("fly-{}-{}b-{}gt-{key:06x}", class, m.blocks(), speed);
    let cap = caption(m, &r);
    let facts = text.lines().find(|l| l.starts_with("# census:")).map(|l| l.trim_start_matches('#').trim().to_string()).unwrap_or_default();
    let mut mm = m.clone();
    mm.clock.design.name = name.clone();
    let body = mm.to_text();
    let mut out = format!("# {cap}\n");
    if !facts.is_empty() {
        out.push_str(&format!("\n# {facts}\n"));
    }
    out.push_str(&body);
    (name, out)
}
