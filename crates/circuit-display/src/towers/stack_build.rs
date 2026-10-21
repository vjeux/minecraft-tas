//! `mctas circuit display stack`: build the compact stack a [`crate::stack`] solution describes.
//! Rings every 2 levels at the column x = z = 0; per side (north shown, the others rotated): the
//! rear block (0,-2) (a redstone block where it is constant 1), the chain torch under it, the
//! outer injection (0,-3) reading the tower X1 at (0,-4) (a repeater, or a subtract comparator
//! whose east side is the repeater (1,-3) reading the tower Y1 at (2,-3)), the diagonal injection
//! (-1,-2) reading X2 at (-2,-2) (its north side the repeater (-1,-3) reading Y2 at (-1,-4)); the
//! ring corners touch the pinwheel towers (-2,-1), (1,-2), (2,1), (-1,2). Every tower but the
//! inputs is a copy fed by a routed line at its base, below the rings.

use super::*;
use crate::stack::{self, Solution};

#[derive(clap::Args)]
pub struct StackOpts {
    #[arg(long)]
    pub out: std::path::PathBuf,
    /// Input roles `p0,p1,p2,p3` (role i is input bit p_i; roles 3 and 2 feed the corners);
    /// with --corners and --order skips the search.
    #[arg(long)]
    pub roles: Option<String>,
    #[arg(long)]
    pub corners: Option<u32>,
    /// Segment order, bottom ring first (`fedcgab`).
    #[arg(long)]
    pub order: Option<String>,
    /// Columns of the role 1 and role 0 inputs `x,z;x,z`.
    #[arg(long, default_value = "-4,-4;4,4")]
    pub inputs: String,
    /// Lines (bottom routing) stay within this half-width + 1.
    #[arg(long, default_value_t = 3)]
    pub radius: i32,
    /// Build orders tried (the first by priority, the others shuffled).
    #[arg(long, default_value_t = 50)]
    pub tries: u32,
    #[arg(long, default_value = "seven-segment-stack")]
    pub name: String,
}

fn rotv(j: usize, (x, z): (i32, i32)) -> (i32, i32) {
    match j % 4 {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    }
}

fn dir_name((dx, dz): (i32, i32)) -> &'static str {
    match (dx, dz) {
        (0, -1) => "north",
        (0, 1) => "south",
        (-1, 0) => "west",
        _ => "east",
    }
}

const X1: (i32, i32) = (0, -4);
const X2: (i32, i32) = (-2, -2);
const Y1: (i32, i32) = (2, -3);
const Y2: (i32, i32) = (-1, -4);
const PINWHEEL: [(i32, i32); 4] = [(-2, -1), (1, -2), (2, 1), (-1, 2)];

fn solution(o: &StackOpts) -> Result<Solution> {
    if let (Some(r), Some(c), Some(ord)) = (&o.roles, o.corners, &o.order) {
        let p: Vec<u32> = r.split(',').map(|x| x.trim().parse()).collect::<std::result::Result<_, _>>()?;
        let perm: [u32; 4] = p.try_into().map_err(|_| anyhow!("--roles takes 4 bits"))?;
        let order: Vec<usize> = ord.chars().map(|ch| stack::NAMES.iter().position(|&n| n == ch).ok_or_else(|| anyhow!("bad segment {ch}"))).collect::<Result<_>>()?;
        return stack::solve(perm, c, &order, &stack::all_towers()).ok_or_else(|| anyhow!("no stack for these roles, corners and order"));
    }
    stack::search_best().ok_or_else(|| anyhow!("no stack"))
}

pub fn run(o: &StackOpts) -> Result<()> {
    let sol = solution(o)?;
    eprint!("{}", stack::describe(&sol));
    let perm = sol.perm;
    let mut last = anyhow!("no attempt");
    let mut built = None;
    for seed in 0..o.tries {
        match towers(o, &sol, seed) {
            Ok(x) => {
                built = Some(x);
                break;
            }
            Err(e) => last = e,
        }
    }
    let Some((mut w, mut design)) = built else { return Err(last) };
    let bottom = w.cells.iter().filter(|(_, k)| matches!(k, K::Dust | K::Rep(_))).map(|(p, _)| p[1]).max().unwrap_or(0);
    let l0 = (bottom + 3 + 3) / 4 * 4;
    let levels: Vec<i32> = (0..7).map(|k| l0 + 2 * k as i32).collect();
    place(&mut w, &sol, &levels, &mut design)?;
    // trim: corners to the top ring, literal towers to their last ring
    for i in 0..w.towers.len() {
        let t = &w.towers[i];
        let label = t.label.clone();
        let mut top = t.uses.iter().copied().max().unwrap_or(t.b);
        if label.starts_with("corner") || PINWHEEL.contains(&(t.x, t.z)) {
            top = levels[6];
        }
        for (j, (_, st)) in sol.sides.iter().enumerate() {
            for (k, s) in st.iter().enumerate() {
                let reads = [(s.outer > 0, X1), (s.diag > 0, X2), (s.y1dust, Y1), (s.y2dust, Y2)];
                if reads.iter().any(|&(r, at)| r && rotv(j, at) == (t.x, t.z)) {
                    top = top.max(levels[k]);
                }
            }
        }
        w.trim(i, top);
    }
    let mut roles_line = format!("# roles {} {} {} {}\n", perm[0], perm[1], perm[2], perm[3]);
    // every comparator's intended output (on where nonzero), for `display check`
    let c0 = stack::corners(sol.assign);
    for (k, &l) in levels.iter().enumerate() {
        let c = if k % 2 == 1 { c0.map(|x| !x) } else { c0 };
        let sides = [(c[0], c[1]), (c[1], c[2]), (c[2], c[3]), (c[3], c[0])];
        for (j, (_, st)) in sol.sides.iter().enumerate() {
            let s = st[k];
            if s.absent {
                continue;
            }
            let d = rotv(j, (0, -1));
            roles_line.push_str(&format!("# comp {} {l} {} {:04x}\n", d.0, d.1, s.value & !sides[j].0 & !sides[j].1));
        }
    }
    write_design(&w, &mut design, &o.out)?;
    let mut text = std::fs::read_to_string(&o.out)?;
    text.push_str(&roles_line);
    std::fs::write(&o.out, text)?;
    crate::settle::run(&o.out, &o.out, 300)
}

/// The inputs and every tower with its routed base line; `seed` > 0 shuffles the build order.
fn towers(o: &StackOpts, sol: &Solution, seed: u32) -> Result<(World, Design)> {
    let perm = sol.perm;
    let c0 = stack::corners(sol.assign);
    let mut w = World::new(o.radius, 15, 5);
    let top_max = 200;
    let mut design = Design { name: o.name.clone(), ..Default::default() };
    design.meta.insert("circuit".into(), "seven-segment 4".into());
    // inputs (truth tables in role bits): roles 3 and 2 where a corner shows the input inverted
    let mut inputs: Vec<((i32, i32), u32)> = Vec::new();
    for role in [3u32, 2] {
        let ci = (0..4).find(|&ci| c0[ci] == !lit(role)).ok_or_else(|| anyhow!("no corner for role {role}"))?;
        inputs.push((PINWHEEL[ci], role));
    }
    let ins = pairs_of(&o.inputs)?;
    inputs.push((ins[0], 1));
    inputs.push((ins[1], 0));
    for &((x, z), role) in &inputs {
        let bit = perm[role as usize];
        w.put_tower(x, z, -2, top_max, lit(role), &format!("a{bit}")).ok_or_else(|| anyhow!("input column {x},{z}"))?;
        design.ports.insert(format!("input.a{bit}"), [x, -2, z]);
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                if dx.abs() + dz.abs() <= 2 && (dx, dz) != (0, 0) {
                    w.cells.entry([x + dx, -1, z + dz]).or_insert(K::Keep);
                }
            }
        }
    }
    // the other two corners, then every literal tower a side uses
    let mut todo: Vec<(u16, (i32, i32), String)> = Vec::new();
    for ci in 0..4 {
        if w.tower_at([PINWHEEL[ci].0, 0, PINWHEEL[ci].1]).is_none() {
            todo.push((c0[ci], PINWHEEL[ci], format!("corner{ci}")));
        }
    }
    for (j, (t, st)) in sol.sides.iter().enumerate() {
        let uses = [(st.iter().any(|s| s.outer > 0), t.x1, X1, "X1"), (st.iter().any(|s| s.diag > 0), t.x2, X2, "X2"), (st.iter().any(|s| s.y1dust), t.y1, Y1, "Y1"), (st.iter().any(|s| s.y2dust), t.y2, Y2, "Y2")];
        for (used, l, at, name) in uses {
            if used {
                todo.push((l.at(0), rotv(j, at), format!("{name}{j}")));
            }
        }
    }
    // several passes: a tower whose source is not built yet waits; the inner towers first (their
    // bases are boxed in once the outer ones stand), then shuffled orders
    todo.sort_by_key(|t| ["corner", "X2", "Y1", "Y2", "X1"].iter().position(|p| t.2.starts_with(p)).unwrap_or(9));
    let mut rng = seed as u64 * 6364136223846793005 + 1442695040888963407;
    for i in (1..todo.len()).rev() {
        if seed == 0 {
            break;
        }
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        todo.swap(i, (rng >> 33) as usize % (i + 1));
    }
    let mut left = todo;
    while !left.is_empty() {
        let mut next = Vec::new();
        let n = left.len();
        for item in left {
            if w.make(item.0, &item.2, top_max, Some(item.1), &[]).is_err() {
                next.push(item);
            }
        }
        if next.len() == n {
            bail!("no line for {}", next.iter().map(|i| i.2.clone()).collect::<Vec<_>>().join(", "));
        }
        left = next;
    }
    Ok((w, design))
}

/// The rings: lamps, comparators, corners, rears, chain torches, injections.
fn place(w: &mut World, sol: &Solution, levels: &[i32], design: &mut Design) -> Result<()> {
    let mut put = |w: &mut World, p: P, kind: K| -> Result<()> {
        if let Some(old) = w.cells.get(&p)
            && *old != kind
        {
            bail!("{p:?} is {old:?}, want {kind:?}");
        }
        w.cells.insert(p, kind);
        Ok(())
    };
    let at = |(x, z): (i32, i32), l: i32| -> P { [x, l, z] };
    for (k, &l) in levels.iter().enumerate() {
        put(w, [0, l, 0], K::Lamp)?;
        put(w, [0, l - 1, 0], K::Keep)?;
        put(w, [0, l + 1, 0], K::Keep)?;
        for (x, z) in [(-1, -1), (1, -1), (1, 1), (-1, 1)] {
            put(w, [x, l, z], K::Corner)?;
            put(w, [x, l - 1, z], K::Stone)?;
        }
        design.ports.insert(format!("output.{}", stack::NAMES[sol.order[k]]), [0, l, 0]);
        for (j, (_, st)) in sol.sides.iter().enumerate() {
            let s = st[k];
            let d = rotv(j, (0, -1));
            if !s.absent {
                put(w, at(d, l), K::Comp(dir_name(d)))?;
            }
            put(w, at(d, l - 1), K::Stone)?;
            let rear = rotv(j, (0, -2));
            put(w, at(rear, l), if s.rblock { K::RBlock } else { K::Stone })?;
            if s.torch {
                put(w, at(rear, l - 1), K::Torch)?;
                if k == 0 {
                    put(w, at(rear, l - 2), K::Stone)?;
                }
            }
            let outer = rotv(j, (0, -3));
            match s.outer {
                1 => put(w, at(outer, l), K::Rep(dir_name(d)))?,
                2 => put(w, at(outer, l), K::Comp(dir_name(d)))?,
                _ => {}
            }
            if s.outer > 0 {
                put(w, at(outer, l - 1), K::Stone)?;
            }
            let diag = rotv(j, (-1, -2));
            let west = rotv(j, (-1, 0));
            match s.diag {
                1 => put(w, at(diag, l), K::Rep(dir_name(west)))?,
                2 => put(w, at(diag, l), K::Comp(dir_name(west)))?,
                _ => {}
            }
            if s.diag > 0 {
                put(w, at(diag, l - 1), K::Stone)?;
            }
            if s.y1dust {
                // the repeater (1,-3) reading Y1 at (2,-3), into the outer comparator's side
                let c = rotv(j, (1, -3));
                put(w, at(c, l), K::Rep(dir_name(rotv(j, (1, 0)))))?;
                put(w, at(c, l - 1), K::Stone)?;
            }
            if s.y2dust {
                // the repeater (-1,-3) reading Y2 at (-1,-4), into the diagonal comparator's side
                let c = rotv(j, (-1, -3));
                put(w, at(c, l), K::Rep(dir_name(d)))?;
                put(w, at(c, l - 1), K::Stone)?;
            }
        }
    }
    Ok(())
}
