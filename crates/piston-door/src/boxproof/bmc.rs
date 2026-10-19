//! The bounded model checker: one SAT problem over a W x H box (one z layer) whose cells are
//! free (one-hot over the palette) or fixed, a lever among candidates, the rest state stable,
//! the open trace (lever on, `t` ticks) ending idle and open, the close trace (lever off, `t`
//! ticks) ending idle at exactly the rest state; FULL seamless (the 12 ring cells hold only
//! stone, obsidian or air at rest and open). Models go to redsim's judge screen (CEGAR) and
//! the passing ones to a design file for the real judge.

use super::Args;
use super::alg::{Alg, FlagMode, Sat};
use super::diff::{cell_of_state, cell_state, fidx, lever_state};
use super::state::*;
use anyhow::{Context, Result, bail};
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::Lit;
use std::time::Instant;

pub struct Opts {
    pub w: i32,
    pub h: i32,
    pub door: (i32, i32),
    pub t: usize,
    pub rounds: usize,
    pub palette: Vec<usize>,
    pub fixed: Vec<Option<Cell<bool>>>,
    pub lever: Option<LeverCand>,
    pub oop: bool,
    pub seamless: bool,
    /// also hold the ring to plain blocks at REST (the brief's stricter reading; `door
    /// properties` FULL only looks at the open state in the door plane)
    pub ring_rest: bool,
    /// FRAME 12/12: all 12 ring cells plain and non-air when open (`--frame12`)
    pub frame12: bool,
    /// the door rests CLOSED with the lever ON (opens on lever off; `--lever-on`)
    pub lever_on_rest: bool,
    pub flags: FlagMode,
    pub max_blocks: Option<usize>,
}

pub fn parse_palette(s: &str) -> Result<Vec<usize>> {
    let mut v = vec![AIR];
    for t in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        v.push(match t {
            "stone" => STONE,
            "obsidian" | "obsi" => OBSI,
            "rblock" | "rb" => RB,
            "glass" => GLASS,
            "note" => NOTE,
            "lamp" => LAMP,
            "bulb" => BULB,
            "comp" | "comparator" => COMP,
            "dropper" => DROP,
            "dust" => DUST,
            "rep" => REP,
            "obs" => OBSV,
            "torch" => TORCH,
            "wtorch" => WTORCH,
            "piston" => PISTON,
            "all" => {
                v.extend([STONE, OBSI, RB, DUST, REP, OBSV, TORCH, WTORCH, PISTON, GLASS, NOTE, LAMP, BULB, COMP, DROP]);
                continue;
            }
            _ => bail!("unknown palette entry {t}"),
        });
    }
    v.sort();
    v.dedup();
    Ok(v)
}

pub fn lever_cands(g: &Geo, oop: bool) -> Vec<LeverCand> {
    let mut v = Vec::new();
    for y in -1..=g.h {
        for x in -1..=g.w {
            let inside = g.idx(x, y);
            if let Some(i) = inside {
                if g.is_door(i) || g.is_ring(i) {
                    continue;
                }
            }
            for d in 0..4 {
                let s = (x + DIRS[d].0, y + DIRS[d].1);
                let Some(si) = g.idx(s.0, s.1) else { continue };
                if g.is_door(si) {
                    continue;
                }
                v.push(LeverCand { pos: (x, y), oop: false, sup: s });
            }
        }
    }
    if oop {
        for i in 0..g.n() {
            if !g.is_door(i) {
                let p = g.xy(i);
                v.push(LeverCand { pos: p, oop: true, sup: p });
            }
        }
    }
    v
}

pub struct Built {
    pub ctx_lsel: Vec<Lit>,
    pub rest: Vec<Cell<Lit>>,
    pub open_end: Vec<Cell<Lit>>,
}

/// Build the whole problem into `sat`.
pub fn build(sat: Sat, o: &Opts, g: &Geo) -> (Sat, Built) {
    let mut sat = sat;
    let n = g.n();
    let lsel: Vec<Lit> = if g.levers.len() == 1 { vec![sat.k(true)] } else { (0..g.levers.len()).map(|_| sat.var()).collect() };
    sat.exactly_one(&lsel);
    let f = sat.k(false);
    let t = sat.k(true);
    // ---- rest placement
    let mut rest: Vec<Cell<Lit>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut c = konst_cell(&sat, AIR);
        c.k[AIR] = f;
        if let Some(fc) = &o.fixed[i] {
            for k in 0..NK {
                c.k[k] = sat.k(fc.k[k]);
            }
            for d in 0..4 {
                c.f[d] = sat.k(fc.f[d]);
            }
            c.st = sat.k(fc.st);
            c.ext = if fc.k[BULB] { sat.var() } else { sat.k(fc.ext) };
            for d in 0..4 {
                c.dl[d] = sat.k(fc.dl[d]);
            }
            c.on = if fc.k[REP] || fc.k[OBSV] || fc.k[TORCH] || fc.k[WTORCH] || fc.k[NOTE] || fc.k[LAMP] || fc.k[COMP] || fc.k[DROP] { sat.var() } else if fc.k[BULB] { sat.k(fc.on) } else { f };
            if fc.k[COMP] {
                for j in 0..15 {
                    c.lvl[j] = sat.var();
                }
            }
            rest.push(c);
            continue;
        }
        let door = g.is_door(i);
        let ring = o.seamless && o.ring_rest && g.is_ring(i);
        let mut ks = Vec::new();
        for k in 0..NK {
            let allowed = if door {
                k == STONE
            } else if ring {
                matches!(k, AIR | STONE | OBSI)
            } else if k == HEAD {
                o.palette.contains(&PISTON)
            } else if k == LEVER {
                g.levers.iter().any(|l| !l.oop && g.idx(l.pos.0, l.pos.1) == Some(i))
            } else {
                k != MOVING && o.palette.contains(&k)
            };
            c.k[k] = if allowed { sat.var() } else { f };
            if allowed {
                ks.push(c.k[k]);
            }
        }
        sat.exactly_one(&ks);
        let needf = sat.or(&[c.k[REP], c.k[OBSV], c.k[WTORCH], c.k[PISTON], c.k[HEAD], c.k[COMP]]);
        let horiz = sat.or(&[c.k[REP], c.k[WTORCH], c.k[COMP]]);
        let fv: Vec<Lit> = (0..4).map(|_| sat.var()).collect();
        for d in 0..4 {
            c.f[d] = fv[d];
            sat.add(&[!fv[d], needf]);
        }
        sat.at_most_one(&fv);
        sat.add(&[!needf, fv[0], fv[1], fv[2], fv[3]]);
        sat.add(&[!horiz, !fv[U]]);
        sat.add(&[!horiz, !fv[D]]);
        let sv = sat.var();
        let stp = sat.or(&[c.k[PISTON], c.k[HEAD]]);
        sat.add(&[!sv, stp]);
        c.st = sv;
        let ev = sat.var();
        let pb = sat.or(&[c.k[PISTON], c.k[BULB]]);
        sat.add(&[!ev, pb]);
        c.ext = ev;
        let dv: Vec<Lit> = (0..4).map(|_| sat.var()).collect();
        for d in 0..4 {
            c.dl[d] = dv[d];
            sat.add(&[!dv[d], c.k[REP]]);
        }
        sat.at_most_one(&dv);
        sat.add(&[!c.k[REP], dv[0], dv[1], dv[2], dv[3]]);
        let onv = sat.var();
        let can_on = sat.or(&[c.k[REP], c.k[OBSV], c.k[TORCH], c.k[WTORCH], c.k[NOTE], c.k[LAMP], c.k[BULB], c.k[COMP], c.k[DROP]]);
        sat.add(&[!onv, can_on]);
        c.on = onv;
        if o.palette.contains(&COMP) {
            for j in 0..15 {
                let v = sat.var();
                sat.add(&[!v, c.k[COMP]]);
                c.lvl[j] = v;
            }
        }
        rest.push(c);
    }
    let mut cx = Ctx { a: sat, g, lsel: lsel.clone(), flags_on: o.flags != FlagMode::Ignore };
    // lever cells: an in-box lever cell holds LEVER exactly when its candidate is chosen
    for i in 0..n {
        let js: Vec<Lit> = g.levers.iter().enumerate().filter(|(_, l)| !l.oop && g.idx(l.pos.0, l.pos.1) == Some(i)).map(|(j, _)| lsel[j]).collect();
        let any = cx.or(&js);
        let k = rest[i].k[LEVER];
        cx.a.add(&[!k, any]);
        cx.a.add(&[k, !any]);
    }
    // lever supports: static sturdy blocks (checked again at every tick boundary below)
    let mut sup_of: Vec<Vec<Lit>> = vec![Vec::new(); n];
    for (j, l) in g.levers.iter().enumerate() {
        let si = g.idx(l.sup.0, l.sup.1).unwrap();
        sup_of[si].push(lsel[j]);
    }
    let is_sup: Vec<Lit> = (0..n).map(|i| cx.or(&sup_of[i].clone())).collect();
    // heads at rest belong to an extended base
    for i in 0..n {
        for d in 0..4 {
            let base = g.nb(i, opp(d));
            let hd = cx.and(&[rest[i].k[HEAD], rest[i].f[d]]);
            match base {
                Some(b) => {
                    let fits = cx.eqv(rest[b].st, rest[i].st);
                    let ok = cx.and(&[rest[b].k[PISTON], rest[b].ext, rest[b].f[d], fits]);
                    cx.a.add(&[!hd, ok]);
                    let be = cx.and(&[rest[b].k[PISTON], rest[b].ext, rest[b].f[d]]);
                    let hd2 = cx.and(&[rest[i].k[HEAD], rest[i].f[d], fits]);
                    cx.a.add(&[!be, hd2]);
                }
                None => cx.a.add(&[!hd]),
            }
        }
        // an extended base facing out of the box has no head cell
        for d in 0..4 {
            if g.nb(i, d).is_none() {
                let x = cx.and(&[rest[i].k[PISTON], rest[i].ext, rest[i].f[d]]);
                cx.a.add(&[!x]);
            }
        }
    }
    // stability of the rest state (lever off, or on with `--lever-on`) and its dust levels
    let lr = o.lever_on_rest;
    let der = cx.derive(&rest, lr);
    for i in 0..n {
        let keep = rest[i].k[COMP];
        for j in 0..15 {
            let cl = cx.and(&[keep, rest[i].lvl[j]]);
            rest[i].lvl[j] = cx.or(&[der.lvl[i][j], cl]);
        }
    }
    for i in 0..n {
        // a bulb's `powered` follows its input; a comparator's output is its rear input
        if cx.a.val(rest[i].k[BULB]) != Some(false) {
            let ni = cx.nin(&rest, &der, i, lr);
            let m = cx.xor(rest[i].ext, ni);
            let bad = cx.and(&[rest[i].k[BULB], m]);
            cx.a.add(&[!bad]);
        }
        // a dropper's `triggered` follows its input (own cell or QC)
        if cx.a.val(rest[i].k[DROP]) != Some(false) {
            let ni = cx.nin(&rest, &der, i, lr);
            let qc = cx.qcin(&rest, &der, i, lr);
            let di = cx.or(&[ni, qc]);
            let m = cx.xor(rest[i].on, di);
            let bad = cx.and(&[rest[i].k[DROP], m]);
            cx.a.add(&[!bad]);
        }
        if cx.a.val(rest[i].k[COMP]) != Some(false) {
            let r = cx.crear(&rest, &der, i, lr);
            for j in 0..15 {
                let m = cx.xor(rest[i].lvl[j], r[j]);
                let bad = cx.and(&[rest[i].k[COMP], m]);
                cx.a.add(&[!bad]);
            }
            let m = cx.xor(rest[i].on, r[0]);
            let bad = cx.and(&[rest[i].k[COMP], m]);
            cx.a.add(&[!bad]);
        }
        let rin = cx.rin(&rest, &der, i, lr);
        let m = cx.xor(rest[i].on, rin);
        let bad = cx.and(&[rest[i].k[REP], m]);
        cx.a.add(&[!bad]);
        let nl = cx.or(&[rest[i].k[NOTE], rest[i].k[LAMP]]);
        if cx.a.val(nl) != Some(false) {
            let ni = cx.nin(&rest, &der, i, lr);
            let m = cx.xor(rest[i].on, ni);
            let bad = cx.and(&[nl, m]);
            cx.a.add(&[!bad]);
        }
        let tin = cx.tin(&rest, &der, i, lr);
        let tk = cx.or(&[rest[i].k[TORCH], rest[i].k[WTORCH]]);
        let tm = cx.eqv(rest[i].on, tin);
        let bad = cx.and(&[tk, tm]);
        cx.a.add(&[!bad]);
        let pw = cx.ppow(&rest, &der, i, lr);
        let npw = cx.not(pw);
        let bad = cx.and(&[rest[i].k[PISTON], rest[i].ext, npw]);
        cx.a.add(&[!bad]);
        let mut res = Vec::new();
        for d in 0..4 {
            let l = cx.line(&rest, i, d);
            res.push(cx.and(&[rest[i].f[d], l.resolves]));
        }
        let res = cx.or(&res);
        let ne = cx.not(rest[i].ext);
        let bad = cx.and(&[rest[i].k[PISTON], ne, pw, res]);
        cx.a.add(&[!bad]);
    }
    let mut rest_ok = Vec::new();
    check_static(&mut cx, &rest, &is_sup, &rest, &mut rest_ok);
    // ---- the traces
    let mut s = cx.flip(&rest, !lr);
    for _ in 0..o.t {
        s = cx.tick(&s, !lr);
        check_static(&mut cx, &s, &is_sup, &rest, &mut rest_ok);
    }
    let open_end = s.clone();
    for i in 0..n {
        idle(&mut cx, &s[i]);
        if g.is_door(i) {
            cx.a.add(&[s[i].k[AIR]]);
        }
        if o.seamless && g.is_ring(i) {
            // FRAME 12/12 (vjeux 10-19 03:36Z): when open, every ring cell holds a plain
            // door-type block (stone: the door blocks' type)
            let ok = if o.frame12 { s[i].k[STONE] } else { cx.or(&[s[i].k[AIR], s[i].k[STONE], s[i].k[OBSI], s[i].k[GLASS]]) };
            cx.a.add(&[ok]);
        }
    }
    let mut s = cx.flip(&s, lr);
    for _ in 0..o.t {
        s = cx.tick(&s, lr);
        check_static(&mut cx, &s, &is_sup, &rest, &mut rest_ok);
    }
    for i in 0..n {
        idle(&mut cx, &s[i]);
        let same = cx.same(&s[i], &rest[i]);
        cx.a.add(&[same]);
        let fs: Vec<Lit> = (0..4).map(|d| cx.eqv(s[i].f[d], rest[i].f[d])).collect();
        for x in fs {
            cx.a.add(&[x]);
        }
        let e = cx.eqv(s[i].st, rest[i].st);
        let both_st = cx.or(&[rest[i].k[PISTON], rest[i].k[HEAD]]);
        let ok = cx.or(&[e, !both_st]);
        cx.a.add(&[ok]);
    }
    if let Some(mb) = o.max_blocks {
        let blocks: Vec<Lit> = (0..n).map(|i| !rest[i].k[AIR]).collect();
        let tot = cx.a.totalizer(&blocks);
        if mb < tot.len() {
            cx.a.add(&[!tot[mb]]);
        }
    }
    let _ = t;
    (cx.a, Built { ctx_lsel: lsel, rest, open_end })
}

fn idle(cx: &mut Ctx<Sat>, c: &Cell<Lit>) {
    for j in 0..8 {
        cx.a.add(&[!c.sch[j]]);
    }
    for e in 0..3 {
        cx.a.add(&[!c.ev[e]]);
    }
    cx.a.add(&[!c.k[MOVING]]);
}

/// Every tick boundary: supports hold, lever supports untouched.
fn check_static(cx: &mut Ctx<Sat>, s: &[Cell<Lit>], is_sup: &[Lit], rest: &[Cell<Lit>], _acc: &mut Vec<Lit>) {
    let g = cx.g;
    for i in 0..g.n() {
        let below = g.nb(i, D);
        let need = cx.or(&[s[i].k[DUST], s[i].k[REP], s[i].k[TORCH], s[i].k[COMP]]);
        let ok = match below {
            Some(b) => {
                let c = s[b];
                cx.sturdy(&c, U)
            }
            None => cx.fl(),
        };
        cx.a.add(&[!need, ok]);
        for fdir in [E, W] {
            let wt = cx.and(&[s[i].k[WTORCH], s[i].f[fdir]]);
            let ok = match g.nb(i, opp(fdir)) {
                Some(b) => {
                    let c = s[b];
                    cx.sturdy(&c, fdir)
                }
                None => cx.fl(),
            };
            cx.a.add(&[!wt, ok]);
        }
        // a lever support keeps its kind and stays sturdy
        let st_ok = cx.or(&[s[i].k[STONE], s[i].k[OBSI], s[i].k[RB], s[i].k[OBSV], s[i].k[GLASS], s[i].k[NOTE], s[i].k[LAMP], s[i].k[BULB], s[i].k[DROP]]);
        cx.a.add(&[!is_sup[i], st_ok]);
        for k in [STONE, OBSI, RB, OBSV, GLASS, NOTE, LAMP, BULB, DROP] {
            let e = cx.eqv(s[i].k[k], rest[i].k[k]);
            cx.a.add(&[!is_sup[i], e]);
        }
    }
}

/// Design text from a solved rest state (`lever_on`: the lever is powered at rest).
pub fn design_text(g: &Geo, cells: &[Cell<bool>], lev: &LeverCand, name: &str, lever_on: bool) -> String {
    let mut pal: Vec<(String, char)> = Vec::new();
    let chars: Vec<char> = "ABCDEFGHIJKMNOPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz0123456789".chars().collect();
    let lstate = lever_state(lev, lever_on);
    let mut ch = |s: &str, pal: &mut Vec<(String, char)>| -> char {
        if s == "minecraft:air" {
            return '.';
        }
        if let Some((_, c)) = pal.iter().find(|(p, _)| p == s) {
            return *c;
        }
        let c = chars[pal.len()];
        pal.push((s.to_string(), c));
        c
    };
    let (mut x0, mut y0, mut x1, mut y1) = (0, 0, g.w - 1, g.h - 1);
    if !lev.oop {
        x0 = x0.min(lev.pos.0);
        y0 = y0.min(lev.pos.1);
        x1 = x1.max(lev.pos.0);
        y1 = y1.max(lev.pos.1);
    }
    let mut layers = String::new();
    let mut states: Vec<String> = (0..g.n()).map(|i| cell_state(&cells[i], &lstate)).collect();
    super::diff::fix_instruments(g.w, g.h, &mut states);
    for y in y0..=y1 {
        let mut row0 = String::new();
        let mut row1 = String::new();
        for x in x0..=x1 {
            let st = match g.idx(x, y) {
                Some(i) => states[i].clone(),
                None => {
                    if !lev.oop && lev.pos == (x, y) {
                        lstate.clone()
                    } else {
                        "minecraft:air".into()
                    }
                }
            };
            row0.push(ch(&st, &mut pal));
            if lev.oop && lev.sup == (x, y) {
                row1.push(ch(&lstate, &mut pal));
            } else {
                row1.push('.');
            }
        }
        layers.push_str(&format!("layer {}:\n  {row0}\n", y - g.door.1));
        if lev.oop {
            layers.push_str(&format!("  {row1}\n"));
        }
    }
    let lp = if lev.oop { (lev.sup.0, lev.sup.1, 1) } else { (lev.pos.0, lev.pos.1, 0) };
    let mut s = format!("name: {name}\nport door: 0 0 0\nport lever: {} {} {}\norigin: {} 0 0\npalette:\n", lp.0 - g.door.0, lp.1 - g.door.1, lp.2, x0 - g.door.0);
    for (p, c) in &pal {
        s.push_str(&format!("  {c} {p}\n"));
    }
    s.push_str(&layers);
    s
}

pub fn read_cells(sat: &Sat, cs: &[Cell<Lit>]) -> Vec<Cell<bool>> {
    cs.iter()
        .map(|c| {
            let v = |l: Lit| sat.value(l);
            Cell {
                k: c.k.map(v),
                f: c.f.map(v),
                st: v(c.st),
                ext: v(c.ext),
                on: v(c.on),
                dl: c.dl.map(v),
                lvl: c.lvl.map(v),
                mk: c.mk.map(v),
                mf: c.mf.map(v),
                mst: v(c.mst),
                mon: v(c.mon),
                mext: v(c.mext),
                msrc: v(c.msrc),
                mold: v(c.mold),
                mnew: v(c.mnew),
                sch: c.sch.map(v),
                ev: c.ev.map(v),
            }
        })
        .collect()
}

/// Fixed cells from a design (cells whose kind is in `kinds`), mapped into the box with the
/// door at `door`.
/// `--approx`: blocks outside the model's palette mapped to the closest modeled block (printed;
/// the redsim screen then runs on the ORIGINAL design, so a found door is still real).
pub fn approx_block(s: &str) -> Option<String> {
    let name = s.split('[').next().unwrap_or(s);
    let prop = |k: &str| s.split(&format!("{k}=")).nth(1).map(|v| v.split([',', ']']).next().unwrap_or("").to_string());
    match name {
        "minecraft:comparator" => Some(s.replace("mode=subtract", "mode=compare")),
        "minecraft:crying_obsidian" => Some("minecraft:obsidian".into()),
        // logs: plain pushable conductors like stone (no leaves in the model)
        n if n.ends_with("_log") || n.ends_with("_wood") => Some("minecraft:stone".into()),
        // an iron trapdoor does not conduct (its open toggle only shows to observers)
        "minecraft:iron_trapdoor" => Some("minecraft:glass".into()),
        "minecraft:redstone_wire" => Some(format!("minecraft:redstone_wire[east=side,north=none,power={},south=none,west=side]", prop("power").unwrap_or("0".into()))),
        _ => None,
    }
}

pub fn fixed_from(path: &str, kinds: &[usize], g: &Geo) -> Result<Vec<Option<Cell<bool>>>> {
    fixed_from2(path, kinds, g, false, &mut Vec::new())
}

/// Fixed cells from a design; `unparsed` collects in-box cells the model cannot represent.
pub fn fixed_from2(path: &str, kinds: &[usize], g: &Geo, approx: bool, unparsed: &mut Vec<usize>) -> Result<Vec<Option<Cell<bool>>>> {
    let d = mctas::design::Design::load(std::path::Path::new(path)).with_context(|| path.to_string())?;
    let door = *d.ports.get("door").context("no door port")?;
    let mut v = vec![None; g.n()];
    for (p, s) in d.block_list() {
        if p[2] != door[2] {
            continue;
        }
        let (x, y) = (p[0] - door[0] + g.door.0, p[1] - door[1] + g.door.1);
        let Some(i) = g.idx(x, y) else { continue };
        let c = match cell_of_state(&s) {
            Some(c) => Some(c),
            None if approx => approx_block(&s).and_then(|r| {
                println!("approx ({x},{y}): {s} -> {r}");
                cell_of_state(&r)
            }),
            None => None,
        };
        let Some(c) = c else {
            unparsed.push(i);
            continue;
        };
        let k = c.k.iter().position(|&b| b).unwrap();
        if kinds.contains(&k) {
            v[i] = Some(c);
        }
    }
    Ok(v)
}

pub fn cli(a: &Args) -> Result<()> {
    let w: i32 = a.get("w", 7);
    let h: i32 = a.get("h", 8);
    let dx: i32 = a.get("dx", (w - 3) / 2);
    let dy: i32 = a.get("dy", (h - 3) / 2);
    let t: usize = a.get("t", 30);
    let rounds: usize = a.get("rounds", 2);
    let palette = parse_palette(a.s("palette").unwrap_or("all"))?;
    let mut g = Geo { w, h, door: (dx, dy), levers: vec![], rounds, split: a.has("split-rounds") };
    let oop = a.has("oop");
    g.levers = lever_cands(&g, oop);
    let mut unparsed: Vec<usize> = Vec::new();
    let fixed = match a.s("fix") {
        Some(p) => {
            let all = a.has("fix-all");
            let kinds: Vec<usize> = if all { (0..NK).collect() } else { parse_palette(a.s("fix-kinds").unwrap_or("piston,stone"))?.into_iter().filter(|&k| k != AIR).chain([HEAD]).collect() };
            let mut v = fixed_from2(p, &kinds, &g, a.has("approx"), &mut unparsed)?;
            if all {
                for (i, c) in v.iter_mut().enumerate() {
                    if c.is_none() {
                        *c = Some(konst_cell(&super::alg::Conc::default(), AIR));
                    }
                    let _ = i;
                }
                // the design's own lever (if it has one)
                let d = mctas::design::Design::load(std::path::Path::new(p))?;
                let door = *d.ports.get("door").context("no door port")?;
                if let Some(&lp) = d.ports.get("lever").filter(|_| !a.has("free-lever")) {
                let ls = d.block_list().into_iter().find(|(q, _)| *q == lp).map(|(_, s)| s).context("no lever block")?;
                let pos = (lp[0] - door[0] + dx, lp[1] - door[1] + dy);
                let face = ls.split("face=").nth(1).map(|s| s.split([',', ']']).next().unwrap_or("")).unwrap_or("");
                let facing = ls.split("facing=").nth(1).map(|s| s.split([',', ']']).next().unwrap_or("")).unwrap_or("");
                let sup = match (face, facing) {
                    ("floor", _) => (pos.0, pos.1 - 1),
                    ("ceiling", _) => (pos.0, pos.1 + 1),
                    (_, "west") => (pos.0 + 1, pos.1),
                    (_, "east") => (pos.0 - 1, pos.1),
                    _ => pos,
                };
                let oop = lp[2] != door[2];
                let cand = if oop { LeverCand { pos: sup, oop: true, sup } } else { LeverCand { pos, oop: false, sup } };
                println!("fixed lever {cand:?}");
                g.levers = vec![cand];
                }
            }
            v
        }
        None => vec![None; g.n()],
    };
    // cells to leave free even under --fix ("x,y;x,y" in box coordinates)
    let mut fixed = fixed;
    // cube-and-conquer over the lever: keep candidates j with j % lever-mod == lever-rem
    if let Some(m) = a.s("lever-mod").and_then(|s| s.parse::<usize>().ok()) {
        let r: usize = a.get("lever-rem", 0);
        if g.levers.len() > 1 {
            g.levers = g.levers.iter().enumerate().filter(|(j, _)| j % m == r).map(|(_, l)| l.clone()).collect();
        }
    }
    let mut freed: Vec<usize> = Vec::new();
    if let Some(fr) = a.s("free") {
        for c in fr.split(';').filter(|s| !s.is_empty()) {
            let (x, y) = c.split_once(',').map(|(a, b)| (a.trim().parse::<i32>().unwrap_or(-1), b.trim().parse::<i32>().unwrap_or(-1))).unwrap_or((-1, -1));
            if let Some(i) = g.idx(x, y) {
                fixed[i] = None;
                freed.push(i);
            }
        }
    }
    // nothing silently dropped: a fixed cell the model cannot represent is an error
    let bad: Vec<(i32, i32)> = unparsed.iter().filter(|i| !freed.contains(i)).map(|&i| g.xy(i)).collect();
    if a.has("fix-all") && !bad.is_empty() {
        bail!("cells outside the model's palette (free them or pass --approx): {bad:?}");
    }
    // the original design, for screening partial placements (`--fix-all`): freed cells only change
    let base = if a.has("fix-all") { a.s("fix").map(|p| mctas::design::Design::load(std::path::Path::new(p))).transpose()? } else { None };
    let o = Opts { w, h, door: (dx, dy), t, rounds, palette, fixed, lever: None, oop, seamless: !a.has("no-seamless"), ring_rest: a.has("ring-rest"), frame12: a.has("frame12"), lever_on_rest: a.has("lever-on"), flags: if a.has("prove") { FlagMode::Forbid } else if a.has("nondet") { FlagMode::Nondet } else { FlagMode::Ignore }, max_blocks: a.s("max-blocks").and_then(|s| s.parse().ok()) };
    let t0 = Instant::now();
    let cfg = match a.s("cfg") {
        Some("sat") => Some(rustsat_cadical::Config::Sat),
        Some("unsat") => Some(rustsat_cadical::Config::Unsat),
        Some("plain") => Some(rustsat_cadical::Config::Plain),
        Some("default") => None,
        _ => if o.flags == FlagMode::Ignore { Some(rustsat_cadical::Config::Sat) } else { Some(rustsat_cadical::Config::Unsat) },
    };
    let (mut sat, b) = build(Sat::with_config_seed(o.flags, cfg, a.s("seed").and_then(|s| s.parse().ok())), &o, &g);
    sat.drop_caches();
    println!("built {}x{} door@{:?} t={} rounds={}: {} vars {} clauses {} flags, {} lever candidates, {:.1}s", w, h, (dx, dy), t, rounds, sat.vars(), sat.clauses, sat.flags.len(), g.levers.len(), t0.elapsed().as_secs_f64());
    let out_dir = a.s("out").unwrap_or("/tmp/boxproof").to_string();
    std::fs::create_dir_all(&out_dir)?;
    let iters: usize = a.get("iters", 50);
    for it in 0..iters {
        let t1 = Instant::now();
        let r = sat.s.solve()?;
        let dt = t1.elapsed().as_secs_f64();
        match r {
            SolverResult::Sat => {
                let rest = read_cells(&sat, &b.rest);
                let j = b.ctx_lsel.iter().position(|&l| sat.value(l)).unwrap();
                let lev = g.levers[j].clone();
                let name = format!("boxproof-{}x{}-{}", w, h, it);
                let mut text = design_text(&g, &rest, &lev, &name, o.lever_on_rest);
                if let Some(base) = &base {
                    // merge: the original design with the freed cells taken from the model
                    std::fs::write(format!("{out_dir}/{name}-model.txt"), &text)?;
                    let mut m = base.clone();
                    m.name = name.clone();
                    let door = m.ports["door"];
                    let at = |i: usize| -> [i32; 3] {
                        let (x, y) = g.xy(i);
                        [x - g.door.0 + door[0], y - g.door.1 + door[1], door[2]]
                    };
                    for &i in &freed {
                        let s = cell_state(&rest[i], "minecraft:lever");
                        if s == "minecraft:air" {
                            m.blocks.remove(&at(i));
                        } else {
                            m.blocks.insert(at(i), s);
                        }
                    }
                    for &i in &freed {
                        let p = at(i);
                        if m.blocks.get(&p).is_some_and(|s| s.starts_with("minecraft:note_block")) {
                            let below = m.blocks.get(&[p[0], p[1] - 1, p[2]]).cloned().unwrap_or_else(|| "minecraft:air".into());
                            let s = m.blocks[&p].replace("instrument=harp", &format!("instrument={}", super::diff::instrument_below(&below)));
                            m.blocks.insert(p, s);
                        }
                    }
                    text = m.to_text();
                }
                let path = format!("{out_dir}/{name}.txt");
                std::fs::write(&path, &text)?;
                let d = mctas::design::Design::parse(&text)?;
                let mut offs = crate::VerifyOptions::default().offsets;
                offs.extend(crate::extra_offsets(28));
                // screen positions: 1 by default while searching (vjeux 03:02Z: positions and
                // rotations are checked once a door exists); `--screen-positions 32` for all
                let np: usize = a.get("screen-positions", 1);
                offs.truncate(np.max(1));
                let sc = crate::redsim_screen::screen_ext(&d, &offs, 3, 400, true, o.lever_on_rest);
                println!("SAT in {dt:.1}s (iter {it}): {path}: redsim screen {sc:?}");
                if sc.passed() {
                    println!("{text}");
                    return Ok(());
                }
                // block this placement
                let mut cl = Vec::new();
                for (i, c) in b.rest.iter().enumerate() {
                    for k in 0..NK {
                        if rest[i].k[k] {
                            cl.push(!c.k[k]);
                        }
                    }
                    for d in 0..4 {
                        if rest[i].f[d] {
                            cl.push(!c.f[d]);
                        }
                    }
                }
                cl.push(!b.ctx_lsel[j]);
                sat.add(&cl);
            }
            SolverResult::Unsat => {
                println!("UNSAT in {dt:.1}s (iter {it}): no door in this model ({}x{}, door at {:?}, t={} per edge, rounds {}, races {})", w, h, (dx, dy), t, rounds, if o.flags == FlagMode::Forbid { "forbidden" } else if o.flags == FlagMode::Nondet { "nondeterministic (either order)" } else { "ignored" });
                return Ok(());
            }
            SolverResult::Interrupted => bail!("interrupted"),
        }
    }
    let _ = fidx;
    Ok(())
}
