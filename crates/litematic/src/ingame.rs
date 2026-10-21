//! `mctas litematic verify-ingame`: each design's .litematic pasted by Litematica ITSELF in the
//! real client (Fabric + MaLiLib + Litematica + the repository's probe mod, `mctas::fabric`),
//! then the world compared with the design block for block, block entities and entities.
//!
//! The designs are laid out in a grid. Two worlds (`--world`):
//! - `server`: the client joins the bridge's server (one frozen sandbox: nothing ticks), the
//!   player becomes a flying creative operator above the grid, and Litematica pastes with
//!   commands (`setblock ... strict`, `fill`, `data modify` with `--nbt place_data_modify`,
//!   `summon <id> x y z`: on a server Litematica summons entities without their data); the
//!   verifier reads the area through the bridge.
//! - `single`: the probe creates a flat creative singleplayer world and freezes its ticks
//!   (`tick freeze`); Litematica pastes directly into the integrated server (all data, entities
//!   included); the probe dumps the area (blocks, block entities, entities with all their saved
//!   data) and the verifier reads the dump.
//!
//! For each design the probe loads the file (`LitematicaSchematic.createFromFile`), places it
//! (`SchematicPlacement.createFor` at the design's origin), waits for Litematica's schematic world
//! and pastes it (`pastePlacementToWorld`).

use crate::nbt::Tag;
use crate::{Meta, design_comments, export, now_ms};
use anyhow::{Context, Result, anyhow, bail};
use mctas::design::Design;
use mctas::sim::{Pos, SandboxOptions, Worker, WorkerConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Where Litematica pastes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum World {
    /// The bridge's server: Litematica's command paste.
    Server,
    /// A singleplayer world: Litematica's direct paste.
    Single,
}

impl World {
    pub fn name(self) -> &'static str {
        match self {
            World::Server => "server",
            World::Single => "singleplayer",
        }
    }
}

/// Options of [`verify_ingame`].
#[derive(Clone, Debug)]
pub struct IngameOptions {
    pub version: String,
    /// Free cells between two designs of the grid.
    pub gap: i32,
    /// Seconds to wait for one paste.
    pub timeout_s: u64,
    /// Litematica's Generic > pasteNbtRestoreBehavior for the pastes: `none` (its default: on a
    /// server, block entities are pasted empty), `place_data_modify`, `place_clone`.
    pub nbt: String,
    /// Litematica's Generic > deduplicateSchematicEntities (off by default in Litematica: a
    /// placement chunk loaded again into its schematic world adds its entities again, and a
    /// server paste summons every copy).
    pub dedupe: bool,
    pub worlds: Vec<World>,
}

impl Default for IngameOptions {
    fn default() -> Self {
        IngameOptions {
            version: mctas::DEFAULT_VERSION.to_string(),
            gap: 4,
            timeout_s: 180,
            nbt: "place_data_modify".to_string(),
            dedupe: true,
            worlds: vec![World::Server, World::Single],
        }
    }
}

/// One design's verdict in one world: `Ok(summary)` or the differences.
pub struct Verdict {
    pub name: String,
    pub world: World,
    pub result: std::result::Result<String, Vec<String>>,
}

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// The design's box (blocks, block entities, entity cells): (lo, hi).
fn bounds(d: &Design) -> (Pos, Pos) {
    let mut cells: Vec<Pos> = d.blocks.keys().copied().collect();
    cells.extend(d.block_entities.keys().copied());
    cells.extend(d.entities.iter().map(|e| [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]));
    let mut lo = cells.first().copied().unwrap_or([0; 3]);
    let mut hi = lo;
    for c in &cells {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k]);
            hi[k] = hi[k].max(c[k]);
        }
    }
    (lo, hi)
}

/// Does the world's data `w` hold everything the design's `d` says (compounds: every key of `d`,
/// recursively; lists: the same length, element by element; numbers by value whatever their
/// width; ids with or without `minecraft:`)?
fn tag_contains(w: &Tag, d: &Tag) -> bool {
    let num = |t: &Tag| -> Option<f64> {
        match t {
            Tag::Byte(v) => Some(*v as f64),
            Tag::Short(v) => Some(*v as f64),
            Tag::Int(v) => Some(*v as f64),
            Tag::Long(v) => Some(*v as f64),
            Tag::Float(v) => Some(*v as f64),
            Tag::Double(v) => Some(*v),
            _ => None,
        }
    };
    let id = |s: &str| s.strip_prefix("minecraft:").unwrap_or(s).to_string();
    match (w, d) {
        (Tag::Compound(a), Tag::Compound(b)) => b.iter().all(|(k, v)| a.iter().any(|(kk, vv)| kk == k && tag_contains(vv, v))),
        (Tag::List(a), Tag::List(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| tag_contains(x, y)),
        (Tag::String(a), Tag::String(b)) => id(a) == id(b),
        _ => match (num(w), num(d)) {
            (Some(a), Some(b)) => a == b,
            _ => w == d,
        },
    }
}

/// An entity as a world shows it.
struct Ent {
    kind: String,
    pos: [f64; 3],
    /// Its saved data (the bridge's leaves out position and motion).
    data: Option<Tag>,
    motion: Option<[f64; 3]>,
    rotation: Option<[f32; 2]>,
}

/// The pasted area: every cell's state, the block entities' data, the entities.
struct View {
    blocks: HashMap<Pos, String>,
    block_entities: HashMap<Pos, String>,
    entities: Vec<Ent>,
}

/// The area through the bridge (the server world).
fn view_bridge(w: &mut Worker, sb: u32, lo: Pos, hi: Pos) -> Result<View> {
    let region = w.get_region(sb, lo, hi)?;
    let blocks = region.iter().map(|(p, s)| (p, s.to_string())).collect();
    let block_entities = w.block_entities(sb, lo, hi)?.into_iter().map(|b| (b.pos, b.nbt)).collect();
    let entities = w
        .entities(sb)?
        .into_iter()
        .map(|x| Ent {
            kind: x.kind,
            pos: x.pos,
            data: crate::nbt::parse_snbt(&x.state).ok(),
            motion: Some(x.motion),
            rotation: Some([x.yaw, x.pitch]),
        })
        .collect();
    Ok(View { blocks, block_entities, entities })
}

/// The probe's dump of the area (the singleplayer world).
fn view_dump(text: &str) -> Result<View> {
    let mut v = View { blocks: HashMap::new(), block_entities: HashMap::new(), entities: Vec::new() };
    let int = |s: &str| s.parse::<i32>().map_err(|e| anyhow!("dump: {s:?}: {e}"));
    for line in text.lines() {
        let mut it = line.splitn(5, ' ');
        match it.next() {
            Some("B") => {
                let (x, y, z) = (int(it.next().unwrap_or(""))?, int(it.next().unwrap_or(""))?, int(it.next().unwrap_or(""))?);
                v.blocks.insert([x, y, z], it.next().unwrap_or("").to_string());
            }
            Some("T") => {
                let (x, y, z) = (int(it.next().unwrap_or(""))?, int(it.next().unwrap_or(""))?, int(it.next().unwrap_or(""))?);
                let rest = it.next().unwrap_or("");
                let snbt = rest.split_once(' ').map(|(_, s)| s).unwrap_or("{}");
                v.block_entities.insert([x, y, z], snbt.to_string());
            }
            Some("N") => {
                let mut f = line.splitn(6, ' ');
                f.next();
                let kind = f.next().unwrap_or("").to_string();
                let num = |s: Option<&str>| s.unwrap_or("").parse::<f64>().map_err(|e| anyhow!("dump entity: {e}"));
                let pos = [num(f.next())?, num(f.next())?, num(f.next())?];
                let data = crate::nbt::parse_snbt(f.next().unwrap_or("{}")).ok();
                v.entities.push(Ent { kind, pos, data, motion: None, rotation: None });
            }
            _ => {}
        }
    }
    Ok(v)
}

/// Compare the pasted area with the design (its box's corner at `origin`): every cell of the box
/// (the design's state or air; a property the design leaves out is the game's default and is not
/// compared), the design's block entities (the world's data holds every key of the design's SNBT:
/// [`tag_contains`]), the design's entities (same type within 1e-6 of the expected position, and,
/// with `entity_data`, their data: Motion, Rotation and every other key the design gives; on a
/// server Litematica summons entities with `summon <id> x y z`, without their data).
fn compare(view: &View, d: &Design, origin: Pos, entity_data: bool) -> std::result::Result<String, Vec<String>> {
    use mctas::blockstate::BlockState;
    let (lo, hi) = bounds(d);
    let shift = [origin[0] - lo[0], origin[1] - lo[1], origin[2] - lo[2]];
    let (wlo, whi) = (add(lo, shift), add(hi, shift));
    let mut diffs = Vec::new();
    let same = |design: &str, world: &str| -> bool {
        match (BlockState::parse(design), BlockState::parse(world)) {
            (Ok(x), Ok(y)) => x.name == y.name && x.props.iter().all(|(k, v)| y.props.get(k) == Some(v)),
            _ => design == world,
        }
    };
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let p = [x, y, z];
                let want = d.blocks.get(&p).map(String::as_str).unwrap_or("minecraft:air");
                let got = view.blocks.get(&add(p, shift)).map(String::as_str).unwrap_or("?");
                if !same(want, got) {
                    diffs.push(format!("block {p:?}: design {want}, world {got}"));
                }
            }
        }
    }
    for (p, snbt) in &d.block_entities {
        match view.block_entities.get(&add(*p, shift)) {
            None => diffs.push(format!("block entity {p:?}: missing in the world")),
            Some(nbt) => match (crate::nbt::parse_snbt(snbt), crate::nbt::parse_snbt(nbt)) {
                (Ok(dt), Ok(wt)) if tag_contains(&wt, &dt) => {}
                (Ok(_), Ok(_)) => diffs.push(format!("block entity {p:?}: design {snbt}, world {nbt}")),
                (Err(e), _) | (_, Err(e)) => diffs.push(format!("block entity {p:?}: unreadable SNBT ({e})")),
            },
        }
    }
    let mut used = vec![false; view.entities.len()];
    for e in &d.entities {
        let want = [e.pos[0] + shift[0] as f64, e.pos[1] + shift[1] as f64, e.pos[2] + shift[2] as f64];
        let kind = if e.kind.contains(':') { e.kind.clone() } else { format!("minecraft:{}", e.kind) };
        let hit = view.entities.iter().enumerate().find(|(i, x)| !used[*i] && x.kind == kind && (0..3).all(|k| (x.pos[k] - want[k]).abs() < 1e-6));
        let Some((i, x)) = hit else {
            let near = view.entities.iter().filter(|x| x.kind == kind).map(|x| format!("{:?}", x.pos)).collect::<Vec<_>>().join(", ");
            diffs.push(format!("entity {kind} at {:?}: not in the world (that type is at: {})", e.pos, if near.is_empty() { "nowhere".into() } else { near }));
            continue;
        };
        used[i] = true;
        if !entity_data {
            continue;
        }
        let Some(Tag::Compound(entries)) = e.nbt.as_deref().and_then(|s| crate::nbt::parse_snbt(s).ok()) else { continue };
        for (k, v) in entries {
            let got: Option<Tag> = match k.as_str() {
                "Pos" | "id" | "UUID" => continue,
                "Motion" if x.motion.is_some() => x.motion.map(|m| Tag::List(m.iter().map(|c| Tag::Double(*c)).collect())),
                "Rotation" if x.rotation.is_some() => x.rotation.map(|r| Tag::List(r.iter().map(|c| Tag::Float(*c)).collect())),
                _ => x.data.as_ref().and_then(|t| t.get(&k)).cloned(),
            };
            if !got.as_ref().is_some_and(|g| tag_contains(g, &v)) {
                diffs.push(format!("entity {kind} at {:?} {k}: design {v:?}, world {}", e.pos, got.map(|g| format!("{g:?}")).unwrap_or_else(|| "(absent)".into())));
            }
        }
    }
    let extra: Vec<String> = view
        .entities
        .iter()
        .enumerate()
        .filter(|(i, x)| !used[*i] && (0..3).all(|k| x.pos[k] >= wlo[k] as f64 && x.pos[k] < (whi[k] + 1) as f64))
        .map(|(_, x)| format!("{} at {:?}", x.kind, x.pos))
        .collect();
    if !extra.is_empty() {
        diffs.push(format!("entities the design does not have: {}", extra.join(", ")));
    }
    if diffs.is_empty() {
        let data = if entity_data || d.entities.is_empty() { "" } else { " (entity data not pasted on a server)" };
        Ok(format!("{} blocks, {} block entities, {} entities identical{data}", d.blocks.len(), d.block_entities.len(), d.entities.len()))
    } else {
        Err(diffs)
    }
}

/// A design laid out for the run: its name, design, origin (its box's corner in the world), its
/// world box and its .litematic.
struct Item {
    name: String,
    design: Design,
    origin: Pos,
    lo: Pos,
    hi: Pos,
    file: PathBuf,
}

/// Paste each design with Litematica in the real client and compare (see the module doc).
pub fn verify_ingame(paths: &[PathBuf], o: &IngameOptions, log: &dyn Fn(&str)) -> Result<Vec<Verdict>> {
    let run = mctas::paths::run_dir().join(format!("litematic-ingame-{}", std::process::id()));
    let schem_dir = run.join("schematics");
    std::fs::create_dir_all(&schem_dir)?;
    // the grid: rows along x up to 96 cells wide, origins at y 1
    let mut items = Vec::new();
    let (mut x, mut z, mut row_depth) = (0i32, 0i32, 0i32);
    for p in paths {
        let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
        let design = Design::parse(&text).with_context(|| format!("parsing {}", p.display()))?;
        let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let (lo, hi) = bounds(&design);
        let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        if x > 0 && x + size[0] > 96 {
            x = 0;
            z += row_depth + o.gap;
            row_depth = 0;
        }
        let origin = [x, 1, z];
        x += size[0] + o.gap;
        row_depth = row_depth.max(size[2]);
        let file = schem_dir.join(format!("{name}.litematic"));
        std::fs::write(&file, export(&design, &Meta::new(&design.name, &design_comments(&text), now_ms()))?)?;
        let whi = [origin[0] + size[0] - 1, origin[1] + size[1] - 1, origin[2] + size[2] - 1];
        items.push(Item { name, design, origin, lo: origin, hi: whi, file });
    }
    let extent = [96 + 16, z + row_depth + 16];
    let client_build = mctas::video::build_client(&o.version)?;
    log("installing Fabric, MaLiLib, Litematica, the probe mod...");
    let fabric = mctas::fabric::install(&o.version, &client_build)?;
    log(&format!(
        "Fabric Loader {}, mods: {}",
        fabric.loader,
        fabric.mods.iter().map(|m| m.file_name().unwrap_or_default().to_string_lossy().to_string()).collect::<Vec<_>>().join(", ")
    ));
    let mut verdicts = Vec::new();
    for &world in &o.worlds {
        let game_dir = run.join(format!("client-{}", world.name()));
        let dir = game_dir.join("mctas-litematica");
        std::fs::create_dir_all(&dir)?;
        let center = [extent[0] as f64 / 2.0, 40.0, extent[1] as f64 / 2.0];
        let mut cfg = format!("nbt {}\ndedupe {}\ncenter {} {} {}\n", o.nbt, o.dedupe, center[0], center[1], center[2]);
        if world == World::Single {
            cfg.push_str("world single\n");
        }
        std::fs::write(dir.join("config.txt"), cfg)?;
        let display = mctas::video::Display::start(854, 480)?;
        log(&format!("{}: starting", world.name()));
        let mut bridge: Option<(Worker, u32)> = None;
        let mut client = if world == World::Server {
            let port = {
                let l = std::net::TcpListener::bind("127.0.0.1:0")?;
                l.local_addr()?.port()
            };
            let wc = WorkerConfig { version: o.version.clone(), listen: Some(port), stderr_log: Some(run.join("server.log")), ..Default::default() };
            let mut w = Worker::spawn(&wc)?;
            let sb = w.create(&SandboxOptions { chunks: [-1, -1, (extent[0] >> 4) + 1, (extent[1] >> 4) + 1], seed: 0 })?;
            let client = mctas::fabric::launch(&client_build, &fabric, &display, Some(port), 854, 480, &game_dir)?;
            bridge = Some((w, sb));
            client
        } else {
            mctas::fabric::launch(&client_build, &fabric, &display, None, 854, 480, &game_dir)?
        };
        // the client in its world
        let deadline = Instant::now() + Duration::from_secs(300);
        if let Some((w, sb)) = bridge.as_mut() {
            while w.viewers()? == 0 {
                if !client.alive() {
                    bail!("the Fabric client exited; see {}", client.log.display());
                }
                if Instant::now() > deadline {
                    bail!("the Fabric client never joined; see {}", client.log.display());
                }
                w.hold(500)?;
            }
            w.view_distance(*sb, 12)?;
            w.camera(*sb, center, 0.0, 90.0)?;
            w.hold(2000)?;
            if w.viewer_mode("creative", true)? == 0 {
                bail!("no viewer to make a creative operator");
            }
            w.hold(3000)?;
        }
        while !dir.join("ready.txt").exists() {
            if !client.alive() {
                bail!("the Fabric client exited; see {}", client.log.display());
            }
            if Instant::now() > deadline {
                bail!("the Fabric client's world never got ready; see {} and {}", client.log.display(), dir.join("probe.log").display());
            }
            match bridge.as_mut() {
                Some((w, _)) => w.hold(500)?,
                None => std::thread::sleep(Duration::from_millis(500)),
            }
        }
        log(&format!("{}: the client is in its world", world.name()));
        let (jobs, done) = (dir.join("jobs.txt"), dir.join("done.txt"));
        for (k, it) in items.iter().enumerate() {
            let id = format!("j{k}");
            let line = format!(
                "{id} {} {} {} {} {} {} {}\n",
                it.file.display(),
                it.origin[0],
                it.origin[1],
                it.origin[2],
                it.hi[0],
                it.hi[1],
                it.hi[2]
            );
            let mut all = std::fs::read_to_string(&jobs).unwrap_or_default();
            all.push_str(&line);
            std::fs::write(&jobs, all)?;
            let t0 = Instant::now();
            let outcome = loop {
                match bridge.as_mut() {
                    Some((w, _)) => w.hold(250)?,
                    None => std::thread::sleep(Duration::from_millis(250)),
                }
                let text = std::fs::read_to_string(&done).unwrap_or_default();
                if let Some(l) = text.lines().find(|l| l.split_whitespace().next() == Some(id.as_str())) {
                    break l[id.len()..].trim().to_string();
                }
                if !client.alive() {
                    bail!("the Fabric client exited; see {}", client.log.display());
                }
                if t0.elapsed() > Duration::from_secs(o.timeout_s) {
                    break "error: no answer from the probe in time".to_string();
                }
            };
            let result = if outcome != "ok" {
                Err(vec![format!("paste: {outcome}")])
            } else {
                let view = match bridge.as_mut() {
                    Some((w, sb)) => {
                        // let the last commands land
                        w.hold(1000)?;
                        view_bridge(w, *sb, it.lo, it.hi)?
                    }
                    None => view_dump(&std::fs::read_to_string(dir.join(format!("dump-{id}.txt")))?)?,
                };
                compare(&view, &it.design, it.origin, world == World::Single)
            };
            match &result {
                Ok(s) => log(&format!("{}: OK {}: {s}", world.name(), it.name)),
                Err(diffs) => log(&format!("{}: DIFF {}: {} differences, first: {}", world.name(), it.name, diffs.len(), diffs[0])),
            }
            verdicts.push(Verdict { name: it.name.clone(), world, result });
        }
        drop(client);
        drop(bridge);
        drop(display);
    }
    Ok(verdicts)
}

/// Every design of the paths (files, or folders: their `*.txt`), sorted.
pub fn collect(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(p)?.flatten().map(|e| e.path()).filter(|q| q.extension().is_some_and(|x| x == "txt")).collect();
            v.sort();
            out.extend(v);
        } else {
            out.push(p.clone());
        }
    }
    if out.is_empty() {
        return Err(anyhow!("no designs"));
    }
    Ok(out)
}
