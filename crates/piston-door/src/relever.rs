//! `door relever`: make working doors record-eligible for REPEATER input (Squid regulation 3.1.3,
//! MC-172213) without touching their machine.
//!
//! With repeater input the lever never changes state and powers nothing by itself: only its
//! support block gets strongly powered, in the scheduled-tick phase. Doors that read the lever
//! itself (an observer watching the lever cell, the lever's weak power into a neighbor) fail.
//! The fix tried here: replace the lever by a block that changes state the moment its power
//! changes (a note block by default) and hang the lever on that block, on each free face. Every
//! observer that watched the lever now sees the substitute toggle in the same tick and phase; the
//! substitute, strongly powered by the lever or by the repeater, powers the lever's old
//! neighbors like the lever's own weak power did (dust and components next to it read a strongly
//! powered conductor). Cost: one block.
//!
//! Each variant is judged like a record (`--extra-offsets` positions, 3 cycles, rotation check)
//! and must pass `search::eligibility` with `--eligible`: repeater input at every judge position
//! (props-dyn's rig, both edges), the hallway check, footsteps for sculk designs, QUART SEAMLESS
//! or better, the lever on the surface with a fixed support. Designs that already pass as they
//! are are reported as `as-is`.

use crate::{DoorSpec, VerifyOptions};
use anyhow::{Context, Result};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker};
use std::collections::HashSet;

/// Substitutes for the lever's cell: blocks whose state follows their power at once, both edges.
pub fn substitute_state(name: &str) -> Option<&'static str> {
    Some(match name {
        "note" => "minecraft:note_block[instrument=harp,note=0,powered=false]",
        "dropper" => "minecraft:dropper[facing=up,triggered=false]",
        "dispenser" => "minecraft:dispenser[facing=up,triggered=false]",
        "lamp" => "minecraft:redstone_lamp[lit=false]",
        "bulb" => "minecraft:waxed_copper_bulb[lit=false,powered=false]",
        // Only with the lever kept on its support (dust cannot hold a lever): the dust reads the
        // strongly powered support and powers the lever's old neighbors.
        "dust" => "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]",
        _ => return None,
    })
}

const FACES: [(Pos, &str, &str); 6] = [
    ([0, 1, 0], "top", "minecraft:lever[face=floor,facing=north,powered=false]"),
    ([0, -1, 0], "bottom", "minecraft:lever[face=ceiling,facing=north,powered=false]"),
    ([0, 0, -1], "north", "minecraft:lever[face=wall,facing=north,powered=false]"),
    ([0, 0, 1], "south", "minecraft:lever[face=wall,facing=south,powered=false]"),
    ([1, 0, 0], "east", "minecraft:lever[face=wall,facing=east,powered=false]"),
    ([-1, 0, 0], "west", "minecraft:lever[face=wall,facing=west,powered=false]"),
];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// One candidate: a label (`as-is`, `<substitute>-<face>` with the lever hung on the substitute,
/// `<substitute>@support-<face>` with the lever moved to another face of its old support) and the
/// design; `fix` = the substitute's cell, whose neighbor-derived state is recomputed once.
#[derive(Clone)]
pub struct Variant {
    pub label: String,
    pub design: Design,
    pub fix: Option<Pos>,
}

/// The design as it is, then every relever variant: each substitute in the lever's cell with the
/// lever hung on each of its free faces (not a design block, not the door, the walkway or the 3x3
/// columns in front of and behind it); and each substitute in the lever's cell with the lever
/// moved to another free face of its old support (the support stays strongly powered, the
/// substitute next to it follows it: a note block toggles, dust also powers the lever's old
/// neighbors).
pub fn variants(d: &Design, subs: &[String]) -> Result<Vec<Variant>> {
    let spec = DoorSpec::from_design(d)?;
    let l = spec.lever;
    let clear: HashSet<Pos> = crate::route::keep_clear(&spec).into_iter().collect();
    let support = d.blocks.get(&l).and_then(|s| crate::properties_dyn::lever_support(s, l));
    // the moved lever keeps the design's rest polarity (either counts, vjeux 2026-10-19)
    let rest_on = d.blocks.get(&l).is_some_and(|s| s.contains("powered=true"));
    let polar = |lever: &str| if rest_on { lever.replace("powered=false", "powered=true") } else { lever.to_string() };
    let mut out = vec![Variant { label: "as-is".into(), design: d.clone(), fix: None }];
    for s in subs {
        let state = substitute_state(s).with_context(|| format!("unknown substitute {s:?} (note, dropper, dispenser, lamp, bulb, dust)"))?;
        // The lever on the substitute.
        if s != "dust" {
            for (f, face, lever) in FACES {
                let p = add(l, f);
                if d.blocks.contains_key(&p) || clear.contains(&p) {
                    continue;
                }
                let mut v = d.clone();
                v.blocks.insert(l, state.to_string());
                v.blocks.insert(p, polar(lever));
                v.ports.insert("lever".into(), p);
                out.push(Variant { label: format!("{s}-{face}"), design: v, fix: Some(l) });
            }
        }
        // The lever on another face of its old support.
        if let Some(sp) = support {
            // Dust needs a floor: a smooth stone under it when the cell below is free (+1 block).
            let below = add(l, [0, -1, 0]);
            let floor = s == "dust" && !d.blocks.contains_key(&below) && !clear.contains(&below);
            for (f, face, lever) in FACES {
                let p = add(sp, f);
                if p == l || d.blocks.contains_key(&p) || clear.contains(&p) || (floor && p == below) {
                    continue;
                }
                let mut v = d.clone();
                v.blocks.insert(l, state.to_string());
                if floor {
                    v.blocks.insert(below, "minecraft:smooth_stone".to_string());
                }
                v.blocks.insert(p, polar(lever));
                v.ports.insert("lever".into(), p);
                out.push(Variant { label: format!("{s}{}@support-{face}", if floor { "+floor" } else { "" }), design: v, fix: Some(l) });
            }
        }
    }
    Ok(out)
}

/// A substitute whose state follows its neighbors (a note block's instrument follows the block
/// below): place the design the way the judge does, recompute it once, write it back.
fn fix_shape(w: &mut Worker, sb: u32, d: &Design, cell: Pos) -> Result<Design> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    w.update(sb, &[cell], "shape")?;
    let r = w.get_region(sb, cell, cell)?;
    let mut out = d.clone();
    if let Some(s) = r.get(cell) {
        out.blocks.insert(cell, s.to_string());
    }
    Ok(out)
}

/// The outcome of one variant.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub label: String,
    pub design: Design,
    /// Ok((lever open, lever close, repeater open, repeater close, seamless tier)) or why not.
    pub verdict: std::result::Result<(u32, u32, u32, u32, String), String>,
}

/// Judge one variant: screen, eligibility at the first position, the full judge, the rotation
/// check, eligibility at every judge position, the SEAMLESS tier.
pub fn judge_variant(w: &mut Worker, v: Variant, judge: &VerifyOptions, screen: &VerifyOptions, need_rotation: bool, seam: Option<crate::properties::SeamReq>) -> Result<Outcome> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<Outcome> {
        let d = match v.fix {
            None => v.design.clone(),
            Some(cell) => fix_shape(w, sb, &v.design, cell)?,
        };
        let verdict = (|| -> std::result::Result<(u32, u32, u32, u32, String), String> {
            let e = |e: anyhow::Error| format!("{e:#}").replace(['\n', '\t'], " ");
            crate::search::screen_in(w, sb, &d, screen).map_err(|x| format!("judge (screen): {}", e(x)))?;
            let first: Vec<Pos> = judge.offsets.iter().take(1).copied().collect();
            crate::search::eligibility(w, sb, &d, true, true, &first).map_err(e)?;
            let rep = crate::verify_in(w, sb, &d, judge).map_err(|x| format!("judge: {}", e(x)))?;
            let (o, c) = crate::search::worst_ticks(&rep);
            if need_rotation {
                if let Some((t, why)) = crate::rotation_check(w, &d, judge).map_err(e)? {
                    return Err(format!("not rotation-safe (turned {} degrees: {why})", t as u32 * 90));
                }
            }
            crate::search::eligibility(w, sb, &d, true, true, &judge.offsets).map_err(e)?;
            if let Some(req) = seam {
                req.check(w, sb, &d).map_err(e)?;
            }
            let (ro, rc) = crate::properties_dyn::eligibility_in(w, sb, &d, &first, 1, false, false).map_err(e)?;
            let p = crate::properties::input_props(w, sb, &d).map_err(e)?;
            let tier = p.get("seamless").cloned().unwrap_or_else(|| "-".into());
            let vol = format!("volume {} ({})", p.get("volume").map(String::as_str).unwrap_or("?"), p.get("volume_dims").map(String::as_str).unwrap_or("?"));
            Ok((o, c, ro, rc, format!("{tier}, {vol}")))
        })();
        Ok(Outcome { label: v.label.clone(), design: d, verdict })
    })();
    let _ = w.drop_sandbox(sb);
    r
}
