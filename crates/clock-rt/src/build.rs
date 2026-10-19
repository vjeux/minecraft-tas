//! A small design builder: components at their cell, each with the floor it needs under it.

use mctas::design::Design;
use std::collections::BTreeMap;

pub type Pos = [i32; 3];

pub fn add(p: Pos, d: Pos) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

pub const DOWN: Pos = [0, -1, 0];

/// Horizontal directions by name (the game's `facing` words).
pub fn dir(name: &str) -> Pos {
    match name {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        _ => panic!("bad direction {name}"),
    }
}

pub fn opposite(name: &str) -> &'static str {
    match name {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        _ => panic!("bad direction {name}"),
    }
}

#[derive(Clone, Debug, Default)]
pub struct B {
    pub name: String,
    pub blocks: BTreeMap<Pos, String>,
    pub ports: BTreeMap<String, Pos>,
    pub meta: BTreeMap<String, String>,
    /// Two different blocks wanted in one cell (the design is impossible).
    pub conflict: bool,
}

impl B {
    pub fn new(name: &str) -> B {
        B { name: name.to_string(), ..Default::default() }
    }

    /// Put a block; a second, different block in the same cell marks a conflict.
    pub fn put(&mut self, p: Pos, s: &str) {
        let s = if s.contains(':') { s.to_string() } else { format!("minecraft:{s}") };
        if let Some(old) = self.blocks.get(&p) {
            if *old != s {
                self.conflict = true;
            }
            return;
        }
        self.blocks.insert(p, s);
    }

    pub fn stone(&mut self, p: Pos) {
        self.put(p, "stone");
    }

    /// The support block under `p` (stone, unless a block is already there).
    pub fn floor(&mut self, p: Pos) {
        let f = add(p, DOWN);
        if !self.blocks.contains_key(&f) {
            self.put(f, "stone");
        }
    }

    pub fn dust(&mut self, p: Pos) {
        self.put(p, "redstone_wire");
        self.floor(p);
    }

    /// A repeater whose INPUT is on side `facing` (the game's facing), output on the other side.
    pub fn rep(&mut self, p: Pos, facing: &str, delay: u8, powered: bool) {
        self.put(p, &format!("repeater[delay={delay},facing={facing},locked=false,powered={powered}]"));
        self.floor(p);
    }

    pub fn cmp(&mut self, p: Pos, facing: &str, subtract: bool, powered: bool) {
        let mode = if subtract { "subtract" } else { "compare" };
        self.put(p, &format!("comparator[facing={facing},mode={mode},powered={powered}]"));
        self.floor(p);
    }

    /// A wall torch hanging on the block at `p - dir(facing)`.
    pub fn wall_torch(&mut self, p: Pos, facing: &str, lit: bool) {
        self.put(p, &format!("redstone_wall_torch[facing={facing},lit={lit}]"));
    }

    /// A standing torch on the block under it (not added).
    pub fn torch(&mut self, p: Pos, lit: bool) {
        self.put(p, &format!("redstone_torch[lit={lit}]"));
    }

    pub fn port(&mut self, name: &str, p: Pos) {
        self.ports.insert(name.to_string(), p);
    }

    pub fn design(&self) -> Design {
        Design { name: self.name.clone(), meta: self.meta.clone(), ports: self.ports.clone(), blocks: self.blocks.clone(), ..Default::default() }
    }
}
