//! A tiny design builder for the analog decoder (crates/circuit-display/src/abuild.rs): blocks
//! by position, components by OUTPUT direction (Minecraft's `facing` is where the input comes
//! from, the opposite), automatic stone supports, ports, hopper/composter constants.

use mctas::design::Design;
use std::collections::BTreeMap;

pub type P = [i32; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum D {
    N,
    S,
    W,
    E,
}

impl D {
    pub fn v(self) -> P {
        match self {
            D::N => [0, 0, -1],
            D::S => [0, 0, 1],
            D::W => [-1, 0, 0],
            D::E => [1, 0, 0],
        }
    }
    pub fn opp(self) -> D {
        match self {
            D::N => D::S,
            D::S => D::N,
            D::W => D::E,
            D::E => D::W,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            D::N => "north",
            D::S => "south",
            D::W => "west",
            D::E => "east",
        }
    }
}

pub fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[derive(Default)]
pub struct B {
    pub blocks: BTreeMap<P, String>,
    pub nbt: BTreeMap<P, String>,
    pub ports: Vec<(String, P)>,
    /// cells that need a sturdy block under them
    pub needs: Vec<P>,
}

impl B {
    pub fn set(&mut self, p: P, s: &str) {
        self.blocks.insert(p, format!("minecraft:{s}"));
    }
    pub fn stone(&mut self, p: P) {
        self.set(p, "stone");
    }
    pub fn dust(&mut self, p: P) {
        self.set(p, "redstone_wire");
        self.needs.push(p);
    }
    /// A comparator whose output points `out`.
    pub fn comp(&mut self, p: P, out: D, subtract: bool) {
        let m = if subtract { "subtract" } else { "compare" };
        self.set(p, &format!("comparator[facing={},mode={m},powered=false]", out.opp().name()));
        self.needs.push(p);
    }
    pub fn rep(&mut self, p: P, out: D, delay: u8) {
        self.set(p, &format!("repeater[delay={delay},facing={}]", out.opp().name()));
        self.needs.push(p);
    }
    pub fn composter(&mut self, p: P, level: u8) {
        self.set(p, &format!("composter[level={level}]"));
    }
    /// A hopper reading `level` (1..=15) on a comparator.
    pub fn hopper(&mut self, p: P, level: u8) {
        self.set(p, "hopper[facing=down,enabled=false]");
        // strength = floor(14 f) + 1 for f > 0, f = items / 320
        let n = (1..=320u32).find(|&n| (14 * n) / 320 + 1 >= level as u32).unwrap_or(320);
        let mut items = Vec::new();
        let mut left = n;
        let mut slot = 0;
        while left > 0 {
            let c = left.min(64);
            items.push(format!("{{Slot:{slot}b,id:\"minecraft:cobblestone\",count:{c}}}"));
            left -= c;
            slot += 1;
        }
        self.nbt.insert(p, format!("{{Items:[{}]}}", items.join(",")));
    }
    pub fn port(&mut self, name: &str, p: P) {
        self.ports.push((name.to_string(), p));
    }
    pub fn supports(&mut self) {
        for p in self.needs.clone() {
            let b = add(p, [0, -1, 0]);
            if !self.blocks.contains_key(&b) {
                self.stone(b);
            }
        }
    }
    pub fn design(&mut self, name: &str, kind: Option<&str>) -> Design {
        self.supports();
        let mut text = format!("name: {name}\n");
        if let Some(k) = kind {
            text.push_str(&format!("circuit: {k}\n"));
        }
        for (n, p) in &self.ports {
            text.push_str(&format!("port {n}: {} {} {}\n", p[0], p[1], p[2]));
        }
        text.push_str("layer 0:\n  .\n");
        let mut d = Design::parse(&text).expect("header");
        d.blocks = self.blocks.clone();
        d.block_entities = self.nbt.clone();
        d
    }
}
