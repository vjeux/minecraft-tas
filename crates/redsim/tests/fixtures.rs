//! Regression fixtures: each `tests/fixtures/*.fixture` is a circuit (`box`, `place update`,
//! `block`, then steps `run n` / `use x y z` / `set x y z <state>`) followed by the REAL GAME's
//! trace recorded by `redsim-fuzz case <case> --fixture <file>`: `frame i` then `cell x y z
//! <state>` for every cell of the box that changed since the previous frame (frame 0 = after
//! placement, in full). redsim must reproduce every frame exactly.

use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::world::{World, flags};
use std::collections::HashMap;

enum Step {
    Run(u32),
    Use(BlockPos),
    Set(BlockPos, Block),
}

struct Fixture {
    lo: [i32; 3],
    hi: [i32; 3],
    update_place: bool,
    blocks: Vec<(BlockPos, Block)>,
    /// container contents set after the blocks (`data merge block` in the game)
    items: Vec<(BlockPos, usize, String, u8)>,
    steps: Vec<Step>,
    /// per frame: changed cells
    frames: Vec<Vec<(BlockPos, String)>>,
}

fn parse(text: &str) -> Fixture {
    let mut f = Fixture { lo: [0; 3], hi: [0; 3], update_place: false, blocks: Vec::new(), items: Vec::new(), steps: Vec::new(), frames: Vec::new() };
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let t: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| t[i].parse::<i32>().unwrap();
        let pos = |i: usize| BlockPos::new(int(i), int(i + 1), int(i + 2));
        match t[0] {
            "box" => {
                f.lo = [int(1), int(2), int(3)];
                f.hi = [int(4), int(5), int(6)];
            }
            "place" => f.update_place = t[1] == "update",
            "block" => f.blocks.push((pos(1), Block::parse(t[4]).unwrap())),
            "item" => f.items.push((pos(1), int(4) as usize, t[5].to_string(), int(6) as u8)),
            "run" => f.steps.push(Step::Run(int(1) as u32)),
            "use" => f.steps.push(Step::Use(pos(1))),
            "set" => f.steps.push(Step::Set(pos(1), Block::parse(t[4]).unwrap())),
            "frame" => f.frames.push(Vec::new()),
            "cell" => f.frames.last_mut().unwrap().push((pos(1), t[4].to_string())),
            other => panic!("bad fixture line {other:?}"),
        }
    }
    f
}

/// Fill the containers (grouped per container, in first-appearance order).
fn fill(w: &mut World, f: &Fixture) {
    use redsim::redstone::container::{Stack, item_id, set_contents};
    let mut groups: Vec<(BlockPos, Vec<(usize, Stack)>)> = Vec::new();
    for (p, slot, item, count) in &f.items {
        let st = Stack { item: item_id(item).expect("known item"), count: *count };
        match groups.iter_mut().find(|(q, _)| q == p) {
            Some((_, v)) => v.push((*slot, st)),
            None => groups.push((*p, vec![(*slot, st)])),
        }
    }
    for (p, v) in groups {
        set_contents(w, p, &v);
    }
}

fn run(f: &Fixture) -> Result<(), String> {
    let m = 6;
    let mut w = World::new(BlockPos::new(f.lo[0] - m, f.lo[1] - m, f.lo[2] - m), [f.hi[0] - f.lo[0] + 1 + 2 * m, f.hi[1] - f.lo[1] + 1 + 2 * m, f.hi[2] - f.lo[2] + 1 + 2 * m]);
    if f.update_place {
        for (p, b) in &f.blocks {
            w.set_block(*p, *b, flags::ALL);
        }
        fill(&mut w, f);
    } else {
        w.place_raw(&f.blocks);
        fill(&mut w, f);
        w.place_updates(&f.blocks);
    }
    let mut expected: HashMap<BlockPos, String> = HashMap::new();
    let mut frame = 0usize;
    let mut check = |w: &World, frame: usize| -> Result<(), String> {
        for (p, s) in &f.frames[frame] {
            expected.insert(*p, s.clone());
        }
        for y in f.lo[1]..=f.hi[1] {
            for z in f.lo[2]..=f.hi[2] {
                for x in f.lo[0]..=f.hi[0] {
                    let p = BlockPos::new(x, y, z);
                    let want = expected.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into());
                    let got = w.get(p).to_string();
                    if got != want {
                        return Err(format!("frame {frame}, {p}: redsim {got}, game {want}"));
                    }
                }
            }
        }
        Ok(())
    };
    check(&w, frame)?;
    for st in &f.steps {
        match st {
            Step::Run(n) => {
                for _ in 0..*n {
                    w.tick();
                    frame += 1;
                    check(&w, frame)?;
                }
            }
            Step::Use(p) => {
                w.use_block(*p);
                frame += 1;
                check(&w, frame)?;
            }
            Step::Set(p, b) => {
                w.set_block(*p, *b, flags::ALL);
                frame += 1;
                check(&w, frame)?;
            }
        }
    }
    Ok(())
}

#[test]
fn fixtures_match_the_game() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "fixture")).collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures in {}", dir.display());
    let mut failures = Vec::new();
    for p in &paths {
        let f = parse(&std::fs::read_to_string(p).unwrap());
        if let Err(e) = run(&f) {
            failures.push(format!("{}: {e}", p.file_name().unwrap().to_string_lossy()));
        }
    }
    assert!(failures.is_empty(), "{} of {} fixtures differ from the game:\n{}", failures.len(), paths.len(), failures.join("\n"));
}
