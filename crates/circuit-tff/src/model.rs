//! A circuit design (docs/circuits.md, decided with clockinfra): the door/clock design format plus
//! `circuit: <kind> [args]`, inputs `port input: x y z [drive]` / `port input.<name>: ...`,
//! outputs `port output: x y z` / `port output.<name>: ...`.

use anyhow::{Context, Result, bail};
use mctas::design::Design;
use std::path::Path;

pub type Pos = [i32; 3];

/// How the judge drives an input cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Drive {
    /// An AIR cell: `minecraft:redstone_block` written with updates (on), air with updates (off).
    Block,
    /// Custom states written with updates.
    States { on: String, off: String },
    /// The design's lever there, toggled with a right-click.
    Lever,
    /// The design's button there, pressed (its own pulse length).
    Button,
}

#[derive(Clone, Debug)]
pub struct Input {
    /// `""` for the single `port input:`, else the part after `input.`.
    pub name: String,
    pub pos: Pos,
    pub drive: Drive,
}

#[derive(Clone, Debug)]
pub struct Circuit {
    pub design: Design,
    pub kind: String,
    pub args: Vec<String>,
    pub inputs: Vec<Input>,
    /// (name, cell): `""` for `port output:`.
    pub outputs: Vec<(String, Pos)>,
}

impl Circuit {
    pub fn load(path: &Path) -> Result<Circuit> {
        let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
        Circuit::from_design(Design::parse(&text).with_context(|| path.display().to_string())?)
    }

    pub fn from_design(design: Design) -> Result<Circuit> {
        let (kind, args) = match design.meta.get("circuit") {
            Some(s) => {
                let mut w = s.split_whitespace().map(|x| x.to_string());
                (w.next().unwrap_or_default(), w.collect())
            }
            None => (String::new(), Vec::new()),
        };
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        for (name, pos) in &design.ports {
            let extra = design.meta.get(&format!("port {name}")).cloned().unwrap_or_default();
            if name == "input" || name.starts_with("input.") {
                let words: Vec<&str> = extra.split_whitespace().collect();
                let drive = match words.as_slice() {
                    [] => Drive::Block,
                    ["lever"] => Drive::Lever,
                    ["button"] => Drive::Button,
                    [on] => Drive::States { on: on.to_string(), off: "minecraft:air".into() },
                    [on, off] => Drive::States { on: on.to_string(), off: off.to_string() },
                    _ => bail!("port {name}: bad drive {extra:?}"),
                };
                inputs.push(Input { name: name.strip_prefix("input").unwrap().trim_start_matches('.').to_string(), pos: *pos, drive });
            } else if name == "output" || name.starts_with("output.") {
                outputs.push((name.strip_prefix("output").unwrap().trim_start_matches('.').to_string(), *pos));
            }
        }
        if inputs.is_empty() {
            bail!("no `port input:`");
        }
        if outputs.is_empty() {
            bail!("no `port output:`");
        }
        for i in &inputs {
            if i.drive == Drive::Block && design.blocks.contains_key(&i.pos) {
                bail!("port input{}: the cell holds a block (a redstone-block input needs an air cell)", if i.name.is_empty() { String::new() } else { format!(".{}", i.name) });
            }
        }
        Ok(Circuit { design, kind, args, inputs, outputs })
    }

    pub fn name(&self) -> &str {
        &self.design.name
    }

    /// The judge's block count: every design block except the input devices (a lever or button
    /// in an input cell).
    pub fn blocks(&self) -> usize {
        self.design.blocks.keys().filter(|p| !self.inputs.iter().any(|i| i.pos == **p)).count()
    }

    pub fn input(&self, name: &str) -> Option<usize> {
        self.inputs.iter().position(|i| i.name == name)
    }

    /// The box of the blocks and the port cells.
    pub fn bounds(&self) -> (Pos, Pos) {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        let mut grow = |p: &Pos| {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        };
        self.design.blocks.keys().for_each(&mut grow);
        self.inputs.iter().for_each(|i| grow(&i.pos));
        self.outputs.iter().for_each(|(_, p)| grow(p));
        (lo, hi)
    }

    /// The family signature: the sorted distinct behavior block names (clock judge's names),
    /// without the input devices.
    pub fn family(&self) -> String {
        let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (p, s) in &self.design.blocks {
            if self.inputs.iter().any(|i| i.pos == *p) {
                continue;
            }
            if let Some(n) = clock::wave::family_name(s) {
                names.insert(n);
            }
        }
        names.into_iter().collect::<Vec<_>>().join("+")
    }
}
