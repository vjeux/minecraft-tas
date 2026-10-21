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
    /// Columns of the role 3 and role 2 inputs (default: on the ring corners' towers).
    #[arg(long)]
    pub corner_inputs: Option<String>,
    /// Lines (bottom routing) stay within this half-width + 1.
    #[arg(long, default_value_t = 3)]
    pub radius: i32,
    /// Build orders tried (the first by priority, the others shuffled).
    #[arg(long, default_value_t = 50)]
    pub tries: u32,
    /// Feed the towers with wall-torch branches (no dust) instead of routed lines.
    #[arg(long)]
    pub forest: bool,
    /// With --forest: the rings stand first, the forest climbs between them.
    #[arg(long)]
    pub rings_first: bool,
    /// With --forest: the longest repeater line the feed may use (0: wall-torch branches only).
    #[arg(long, default_value_t = 4)]
    pub lines: i32,
    /// Highest base level of a fed tower.
    #[arg(long, default_value_t = 24)]
    pub y_max: i32,
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

/// `--inputs` and `--corner-inputs` may list alternatives separated by `|`: every combination is
/// built and the smallest box (volume, then blocks) is written to `--out`.
pub fn run(o: &StackOpts) -> Result<()> {
    let ins: Vec<String> = o.inputs.split('|').map(String::from).collect();
    let cis: Vec<Option<String>> = match &o.corner_inputs {
        Some(c) => c.split('|').map(|s| if s == "pinwheel" { None } else { Some(s.to_string()) }).collect(),
        None => vec![None],
    };
    if ins.len() == 1 && cis.len() == 1 {
        return run_one(o).map(|_| ());
    }
    let tmp = o.out.with_extension("sweep.txt");
    let mut best: Option<((i64, usize), String)> = None;
    for i in &ins {
        for c in &cis {
            let oo = StackOpts { out: tmp.clone(), inputs: i.clone(), corner_inputs: c.clone(), roles: o.roles.clone(), order: o.order.clone(), name: o.name.clone(), ..*o };
            match run_one(&oo) {
                Ok(key) => {
                    eprintln!("inputs {i} corner inputs {c:?}: volume {} blocks {}", key.0, key.1);
                    if best.as_ref().is_none_or(|b| key < b.0) {
                        best = Some((key, std::fs::read_to_string(&tmp)?));
                    }
                }
                Err(e) => eprintln!("inputs {i} corner inputs {c:?}: {e}"),
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
    let (key, text) = best.ok_or_else(|| anyhow!("no combination builds"))?;
    std::fs::write(&o.out, text)?;
    println!("{}: best volume {} blocks {}", o.out.display(), key.0, key.1);
    Ok(())
}

fn run_one(o: &StackOpts) -> Result<(i64, usize)> {
    let sol = solution(o)?;
    eprint!("{}", stack::describe(&sol));
    let perm = sol.perm;
    let mut last = anyhow!("no attempt");
    let mut built: Option<(World, Design, i32)> = None;
    let mut best_l0 = i32::MAX;
    let ress: &[i32] = if o.forest { &[0, 2] } else { &[0] };
    let mut placed = false;
    if o.forest && o.rings_first {
        // the lowest first ring whose towers all grow in time
        'l0: for l0 in (2..=o.y_max).step_by(2) {
            for seed in 0..o.tries {
                match towers(o, &sol, seed, l0 % 4, Some(l0)) {
                    Ok((w, d)) => {
                        built = Some((w, d, l0 % 4));
                        best_l0 = l0;
                        placed = true;
                        break 'l0;
                    }
                    Err(e) => {
                        if seed == 0 && std::env::var("DISPLAY_DEBUG").is_ok() {
                            eprintln!("first ring {l0}: {e}");
                        }
                        last = e;
                    }
                }
            }
        }
    } else {
        for seed in 0..o.tries {
            for &res in ress {
                match towers(o, &sol, seed, res, None) {
                    Ok((w, d)) => {
                        // the lowest first ring wins: 3 levels over the feed, at `res` mod 4
                        let need = feed_top(&w) + 3;
                        let l0 = need + (res - need).rem_euclid(4);
                        if l0 < best_l0 {
                            best_l0 = l0;
                            built = Some((w, d, res));
                        }
                    }
                    Err(e) => last = e,
                }
            }
        }
    }
    let Some((mut w, mut design, _res)) = built else { return Err(last) };
    let l0 = best_l0;
    eprintln!("first ring at level {l0}");
    let levels: Vec<i32> = (0..7).map(|k| l0 + 2 * k as i32).collect();
    // towers no ring reads (the role 1 and 0 inputs, relays) end at their last branch before the
    // rings go up over them
    let ring_cols: HashSet<(i32, i32)> = PINWHEEL.iter().copied().chain((0..4).flat_map(|j| [X1, X2, Y1, Y2].map(|a| rotv(j, a)))).collect();
    for i in 0..w.towers.len() {
        let t = &w.towers[i];
        if !ring_cols.contains(&(t.x, t.z)) {
            let top = t.uses.iter().copied().max().unwrap_or(t.b);
            w.trim(i, top);
        }
    }
    if !placed {
        place(&mut w, &sol, &levels, &mut design)?;
    }
    // trim: corners to the top ring, literal towers to their last ring
    for i in 0..w.towers.len() {
        let t = &w.towers[i];
        let label = t.label.clone();
        let mut top = t.uses.iter().copied().max().unwrap_or(t.b);
        if !label.ends_with('~') && (label.starts_with("corner") || PINWHEEL.contains(&(t.x, t.z))) {
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
    crate::settle::run(&o.out, &o.out, 300)?;
    let d = Design::load(&o.out)?;
    let (lo, hi) = d.bounds().ok_or_else(|| anyhow!("empty design"))?;
    let vol = (0..3).map(|i| (hi[i] - lo[i] + 1) as i64).product();
    Ok((vol, d.block_count()))
}

/// The inputs and every tower with its base feed; `seed` > 0 shuffles the build order; `res`: the
/// ring levels mod 4 (0 or 2), where every tower must show its ring-0 value. With `rings` (the
/// first ring level), the rings stand first and the forest climbs between them: a tower only has
/// to be up by the first ring reading it.
fn towers(o: &StackOpts, sol: &Solution, seed: u32, res: i32, rings: Option<i32>) -> Result<(World, Design)> {
    let perm = sol.perm;
    let c0 = stack::corners(sol.assign);
    let mut w = World::new(o.radius, 15, 5);
    let top_max = 200;
    let mut design = Design { name: o.name.clone(), ..Default::default() };
    design.meta.insert("circuit".into(), "seven-segment 4".into());
    // inputs (truth tables in role bits): roles 3 and 2 where a corner shows the input inverted
    let mut inputs: Vec<((i32, i32), u32)> = Vec::new();
    let custom = o.corner_inputs.as_deref().map(pairs_of).transpose()?;
    for (i, role) in [3u32, 2].into_iter().enumerate() {
        if let Some(c) = &custom {
            inputs.push((c[i], role));
            continue;
        }
        // an input tower (base -2) shows its input inverted at levels 0 mod 4, plain at 2 mod 4
        let shown = if res == 0 { !lit(role) } else { lit(role) };
        let ci = (0..4).find(|&ci| c0[ci] == shown).ok_or_else(|| anyhow!("no corner for role {role}"))?;
        inputs.push((PINWHEEL[ci], role));
    }
    let ins = pairs_of(&o.inputs)?;
    inputs.push((ins[0], 1));
    inputs.push((ins[1], 0));
    for (i, &((x, z), role)) in inputs.iter().enumerate() {
        let bit = perm[role as usize];
        // the role 1 and 0 inputs stand under the rings: they end below them
        let top = match rings {
            Some(l0) if i >= 2 || !PINWHEEL.contains(&(x, z)) => l0 - 3,
            _ => top_max,
        };
        w.put_tower(x, z, -2, top, lit(role), &format!("a{bit}")).ok_or_else(|| anyhow!("input column {x},{z}"))?;
        design.ports.insert(format!("input.a{bit}"), [x, -2, z]);
    }
    // the judge drives each input from the first side (N, S, W, E) whose repeater and switch cells
    // are free (all below the box): keep the cells above them clear
    let cols: Vec<(i32, i32)> = inputs.iter().map(|i| i.0).collect();
    for &(x, z) in &cols {
        let side = [(0, -1), (0, 1), (-1, 0), (1, 0)].into_iter().find(|&(dx, dz)| !cols.contains(&(x + dx, z + dz)) && !cols.contains(&(x + 2 * dx, z + 2 * dz)));
        let (dx, dz) = side.ok_or_else(|| anyhow!("input {x},{z} has no free side"))?;
        for k in 1..=2 {
            if w.cells.contains_key(&[x + k * dx, -1, z + k * dz]) {
                bail!("input {x},{z}: its driver touches a design block");
            }
            w.cells.insert([x + k * dx, -1, z + k * dz], K::Keep);
        }
    }
    // the other two corners, then every literal tower a side uses (with the first ring reading it)
    let mut todo: Vec<(u16, (i32, i32), String, usize)> = Vec::new();
    for ci in 0..4 {
        if w.tower_at([PINWHEEL[ci].0, 0, PINWHEEL[ci].1]).is_none() {
            todo.push((c0[ci], PINWHEEL[ci], format!("corner{ci}"), 0));
        }
    }
    for (j, (t, st)) in sol.sides.iter().enumerate() {
        let first = |p: &dyn Fn(&stack::Step) -> bool| st.iter().position(p);
        let uses = [(first(&|s| s.outer > 0), t.x1, X1, "X1"), (first(&|s| s.diag > 0), t.x2, X2, "X2"), (first(&|s| s.y1dust), t.y1, Y1, "Y1"), (first(&|s| s.y2dust), t.y2, Y2, "Y2")];
        for (used, l, at, name) in uses {
            if let Some(k) = used {
                todo.push((l.at(0), rotv(j, at), format!("{name}{j}"), k));
            }
        }
    }
    if let Some(l0) = rings {
        let levels: Vec<i32> = (0..7).map(|k| l0 + 2 * k as i32).collect();
        place(&mut w, sol, &levels, &mut design)?;
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
    if o.forest {
        // wall-torch branches: the targets in order, each kept clear until its turn
        let mut avoid: HashSet<(i32, i32)> = left.iter().map(|t| t.1).collect();
        for item in &left {
            avoid.remove(&item.1);
            let y_max = match rings {
                Some(l0) => l0 + 2 * item.3 as i32 - 1,
                None => o.y_max,
            };
            let top = rings.map(|l0| l0 + 12).unwrap_or(200);
            super::forest::grow(&mut w, item.0, item.1, &item.2, &avoid, y_max, o.radius + 1, res, top, o.lines)?;
        }
        return Ok((w, design));
    }
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

/// The highest level the bottom feed uses: lines, wall torches, tower bases, relay columns.
fn feed_top(w: &World) -> i32 {
    let cells = w.cells.iter().filter(|(_, k)| matches!(k, K::Dust | K::Rep(_) | K::WTorch(_))).map(|(p, _)| p[1]);
    let towers = w.towers.iter().map(|t| if t.label.ends_with('~') { t.top } else { t.b });
    cells.chain(towers).max().unwrap_or(0)
}
