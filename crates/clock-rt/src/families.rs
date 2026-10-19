//! Parametric families of redstone-tick clocks.
//!
//! **U-loop** (`uloop`): two rows of cells at y = 1 (floors at y = 0) closed into a loop:
//!
//! ```text
//!   z=-1:  H  t1 t2 .. tk  F      top row: signal runs east; F a block the last top cell powers
//!   z= 0:  A  b1 b2 .. bk  L      bottom row: signal runs west into A; L the tail beside F
//! ```
//!
//! The HEAD (column 0) is a TORCH (a wall torch H on block A, facing north: an inversion; the
//! first top cell reads it directly) or a CORNER (A a plain block the last bottom cell powers, H a
//! dust beside it feeding the top row). The TAIL (column k+1, bottom) is a DUST beside F (feeds the
//! first bottom cell) or a TORCH hanging on F (an inversion; the first bottom cell reads it).
//! Row cells: a repeater (delay 1-4, optionally powered at placement: a loaded pulse), a
//! comparator (compare mode), dust, a block, or a wall torch hanging on the previous cell's block
//! (facing along the row: an inversion).
//! Blocks: head 2 (torch) or 3 (corner), tail 2 (dust) or 1 (torch), F 1, a repeater / comparator
//! / dust cell 2, a block or torch cell 1.
//! - odd number of torches: an inverter loop, period 2 x loop delay, 50 % duty;
//! - even: a pulse ring, period = loop delay (one pulse), the pulses set by the placement state.
//! Torches toggling more than 7 times in 60 gt burn out (a torch inverter loop of period 8: 220).

use crate::build::B;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cell {
    /// A repeater: delay setting 1-4, powered at placement.
    Rep { d: u8, on: bool },
    /// A compare-mode comparator.
    Cmp,
    /// Redstone dust.
    Dust,
    /// A plain block (stone).
    Block,
    /// A wall torch hanging on the previous cell (lit at placement or not).
    Torch { lit: bool },
}

impl Cell {
    /// `1`..`4` repeater (`*` = powered at placement), `c` comparator, `d` (or `0`) dust,
    /// `b` block, `T` / `t` wall torch lit / unlit.
    pub fn parse(s: &str) -> anyhow::Result<Cell> {
        let (core, on) = match s.strip_suffix('*') {
            Some(c) => (c, true),
            None => (s, false),
        };
        Ok(match core {
            "c" => Cell::Cmp,
            "d" | "0" => Cell::Dust,
            "b" => Cell::Block,
            "T" => Cell::Torch { lit: true },
            "t" => Cell::Torch { lit: false },
            n => {
                let d: u8 = n.parse().map_err(|_| anyhow::anyhow!("bad cell {s:?}: 1-4[*], c, d, b, T or t"))?;
                anyhow::ensure!((1..=4).contains(&d), "repeater delay 1-4, got {d}");
                Cell::Rep { d, on }
            }
        })
    }

    pub fn label(&self) -> String {
        match *self {
            Cell::Rep { d, on } => format!("{d}{}", if on { "*" } else { "" }),
            Cell::Cmp => "c".into(),
            Cell::Dust => "d".into(),
            Cell::Block => "b".into(),
            Cell::Torch { lit: true } => "T".into(),
            Cell::Torch { lit: false } => "t".into(),
        }
    }

    /// Delay in redstone ticks (2 gt each).
    pub fn rt(&self) -> u32 {
        match *self {
            Cell::Rep { d, .. } => d as u32,
            Cell::Cmp | Cell::Torch { .. } => 1,
            Cell::Dust | Cell::Block => 0,
        }
    }

    pub fn blocks(&self) -> usize {
        match *self {
            Cell::Rep { .. } | Cell::Cmp | Cell::Dust => 2,
            Cell::Block | Cell::Torch { .. } => 1,
        }
    }
}

pub fn parse_row(s: &str) -> anyhow::Result<Vec<Cell>> {
    s.split(',').filter(|t| !t.is_empty()).map(Cell::parse).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Head {
    Torch { lit: bool },
    Corner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tail {
    Dust,
    Torch { lit: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ULoop {
    pub head: Head,
    pub top: Vec<Cell>,
    pub bottom: Vec<Cell>,
    pub tail: Tail,
}

impl ULoop {
    pub fn label(&self) -> String {
        let h = match self.head {
            Head::Torch { lit: true } => "T",
            Head::Torch { lit: false } => "t",
            Head::Corner => "C",
        };
        let t = match self.tail {
            Tail::Dust => "d",
            Tail::Torch { lit: true } => "T",
            Tail::Torch { lit: false } => "t",
        };
        let row = |r: &[Cell]| r.iter().map(|c| c.label()).collect::<Vec<_>>().join(",");
        format!("{h}-{}-{}-{t}", row(&self.top), row(&self.bottom))
    }

    /// Torches in the loop (the inversions).
    pub fn torches(&self) -> usize {
        let h = matches!(self.head, Head::Torch { .. }) as usize;
        let t = matches!(self.tail, Tail::Torch { .. }) as usize;
        h + t + self.top.iter().chain(&self.bottom).filter(|c| matches!(c, Cell::Torch { .. })).count()
    }

    /// Loop delay in redstone ticks.
    pub fn loop_rt(&self) -> u32 {
        let h = matches!(self.head, Head::Torch { .. }) as u32;
        let t = matches!(self.tail, Tail::Torch { .. }) as u32;
        h + t + self.top.iter().chain(&self.bottom).map(|c| c.rt()).sum::<u32>()
    }

    /// One line on how it works (the catalog's caption: the design's leading comment).
    pub fn caption(&self) -> String {
        let cell = |c: &Cell| match *c {
            Cell::Rep { d, on } => format!("delay-{d} repeater{}", if on { " (powered at placement)" } else { "" }),
            Cell::Cmp => "comparator".into(),
            Cell::Dust => "dust".into(),
            Cell::Block => "block".into(),
            Cell::Torch { lit } => format!("wall torch{}", if lit { "" } else { " (unlit at placement)" }),
        };
        let row = |r: &[Cell]| r.iter().map(cell).collect::<Vec<_>>().join(", ");
        let head = match self.head {
            Head::Torch { lit } => format!("a wall torch on block A{}", if lit { "" } else { " (unlit at placement)" }),
            Head::Corner => "block A + dust beside it".into(),
        };
        let tail = match self.tail {
            Tail::Dust => "dust beside F".into(),
            Tail::Torch { lit } => format!("a wall torch on F{}", if lit { "" } else { " (unlit at placement)" }),
        };
        let t = self.torches();
        let kind = if t % 2 == 1 {
            format!("{t} torch(es) = an inverter loop of delay {} gt (period 2 x delay)", 2 * self.loop_rt())
        } else {
            format!("{t} torches = a pulse ring of loop delay {} gt", 2 * self.loop_rt())
        };
        format!("U-loop (clk-rt): {head} -> top row [{}] -> block F -> {tail} -> bottom row [{}] -> A; {kind}", row(&self.top), row(&self.bottom))
    }

    pub fn build(&self, name: &str) -> anyhow::Result<B> {
        let k = self.top.len() as i32;
        anyhow::ensure!(k >= 1 && self.bottom.len() as i32 == k, "the two rows need the same length >= 1");
        let mut b = B::new(name);
        let y = 1;
        match self.head {
            Head::Torch { lit } => {
                b.stone([0, y, 0]);
                b.wall_torch([0, y, -1], "north", lit);
                b.port("output", [0, y, -1]);
            }
            Head::Corner => {
                b.stone([0, y, 0]);
                b.dust([0, y, -1]);
                b.port("output", [0, y, -1]);
            }
        }
        // `input`: the side the signal comes from (the game's facing for diodes); a torch faces
        // away from its block, i.e. along the row.
        let place = |b: &mut B, x: i32, z: i32, c: Cell, input: &str, along: &str| match c {
            Cell::Rep { d, on } => b.rep([x, y, z], input, d, on),
            Cell::Cmp => b.cmp([x, y, z], input, false, false),
            Cell::Dust => b.dust([x, y, z]),
            Cell::Block => b.stone([x, y, z]),
            Cell::Torch { lit } => b.wall_torch([x, y, z], along, lit),
        };
        for (i, c) in self.top.iter().enumerate() {
            place(&mut b, 1 + i as i32, -1, *c, "west", "east");
        }
        for (i, c) in self.bottom.iter().enumerate() {
            place(&mut b, 1 + i as i32, 0, *c, "east", "west");
        }
        b.stone([k + 1, y, -1]);
        match self.tail {
            Tail::Dust => b.dust([k + 1, y, 0]),
            Tail::Torch { lit } => b.wall_torch([k + 1, y, 0], "south", lit),
        }
        Ok(b)
    }
}
