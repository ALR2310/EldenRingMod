//! Spirit AI tweaks, on the `NpcThinkParam` rows spirits use (the AI:
//! aggressiveness, sight, battle start distance, ...). Hot reload, no F5 for
//! the files below:
//!
//! - `ImproveSenses` / `ImproveAggression` / `ImproveFollow` (ini keys,
//!   default `true`): built-in bundles of those fields, see [SENSES],
//!   [AGGRESSION] and [FOLLOW]. Values were taken from the Age of Spirit
//!   mod's regulation (its spirits notice enemies from farther away, engage
//!   together and keep up with the player) and tried in game 2026-10-09.
//! - `SpiritThinkParam.ini` and `SpiritParam.ini` (optional, experimental):
//!   set any scalar field / bit flag of the `NpcThinkParam` / `NpcParam` rows
//!   spirits use, so tweaks can be tried in-game without rebuilding the DLL.
//!   They are written after the bundles and win over them.
//!
//! The files live next to the DLL, are never created and are only read if they exist. One
//! `Name=value` per line, `;`/`#` comments. `Name` is the
//! field name as in Smithbox's parentheses (`TeamAttackEffectivity`,
//! `BattleStartDist`, `isGuard_Act`) or the Rust one
//! (`team_attack_effectivity`) - case and underscores are ignored when
//! matching. The file is checked once a second; saving it re-applies it.
//! Spirits already out keep the old values: re-summon them.
//!
//! Which rows: every `npcThinkParamId` / `npcThinkParamId_ridden` of
//! `BuddyParam` (the rows a spirit spawns with). Rows are addressed by index
//! (`common::params`), never through the runtime lookup table.
//!
//! `SpiritParam.ini` (NpcParam: fields as in Smithbox, e.g. `fallDamageDump`)
//! hits the `NpcParam` rows of the same spirits (`npcParamId` /
//! `npcParamId_ridden`). In `SpiritThinkParam.ini` a `[Row id, ...]` section
//! applies to the listed `NpcThinkParam` row IDs only; any other section header
//! goes back to the global lines. Same snapshot / restore logic for both.
//!
//! The first time it runs, each target row's bytes are saved; every later
//! apply starts from that snapshot and writes the listed fields, so a line
//! removed from the file goes back to the game's value (same baseline
//! pattern as `ghost_color.rs`).
//!
//! The field table in `think_fields.rs` was generated from
//! `NPC_THINK_PARAM_ST` (fromsoftware-rs): scalar fields and single-bit flags
//! (`enableNaviFlg_*`, `isNoAvoidHugeEnemy`, ...; value 0 or 1).

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime};

use eldenring::cs::{BuddyParam, CSTaskGroupIndex, NpcParam, NpcThinkParam, SoloParam, SoloParamRepository};
use eldenring::param::{BUDDY_PARAM_ST, NPC_PARAM_ST, NPC_THINK_PARAM_ST};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

mod think_fields {
    include!("think_fields.rs");
}
mod npc_fields {
    include!("npc_fields.rs");
}
use think_fields::{FIELDS, ROW_SIZE};

const THINK_FILE: &str = "SpiritThinkParam.ini";
const NPC_FILE: &str = "SpiritParam.ini";
const TICK_INTERVAL_MS: f64 = 1000.0;

const _: () = assert!(size_of::<NPC_THINK_PARAM_ST>() == ROW_SIZE);
const _: () = assert!(size_of::<NPC_PARAM_ST>() == npc_fields::ROW_SIZE);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    /// One bit (0-based) of the byte at the offset; value 0 or 1.
    Bit(u8),
}

/// How an override combines with the row's own (original) value: a bundle
/// must not undo what another mod's regulation already did, so it only moves a
/// value in the improving direction.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    /// Always write the value (every line of the files).
    Set,
    /// Write the value if it is larger than the original.
    Max,
    /// Write the value if it is smaller than the original.
    Min,
}

/// One validated line of the file, or one bundle entry: field name, byte
/// offset, type, value and how it combines with the original value.
struct Override {
    name: &'static str,
    offset: usize,
    kind: Kind,
    value: f64,
    mode: Mode,
}

use Mode::{Max, Min, Set};

/// `ImproveSenses`: spirits notice enemies from farther away and remember
/// them longer - sight, smell and hearing ranges / memory, and the wider
/// vision used while fighting. (Name, value, mode.)
const SENSES: &[(&str, f64, Mode)] = &[
    ("searchEye_dist", 15.0, Max),
    ("eye_dist", 15.0, Max),
    ("searchEye_angX", 180.0, Max),
    ("searchEye_angY", 180.0, Max),
    ("SightTargetForgetTime", 3.0, Max),
    ("nose_dist", 5.0, Max),
    ("searchTargetLv1ForgetTime", 5.0, Max),
    ("searchTargetLv2ForgetTime", 5.0, Max),
    ("MemoryTargetForgetTime", 5.0, Max),
    ("ear_dist", 5.0, Max),
    ("SoundTargetForgetTime", 5.0, Max),
    ("isUpdateBattleSight", 1.0, Max),
    ("battleEye_updateDist", 60.0, Max),
    ("battleEye_updateAngX", 180.0, Max),
    ("battleEye_updateAngY", 180.0, Max),
];

/// `ImproveAggression`: spirits start fighting from farther away, attack
/// together instead of circling, guard, and call for help.
const AGGRESSION: &[(&str, f64, Mode)] = &[
    ("TeamAttackEffectivity", 100.0, Max),
    ("BattleStartDist", 15.0, Max),
    ("callHelp_CallValidMinDistTarget", 5.0, Set),
    ("callHelp_CallValidRange", 15.0, Max),
    ("platoonReplyAddRandomTime", 2.0, Set),
    ("isGuard_Act", 1.0, Max),
    ("thinkAttr_doAdmirer", 1.0, Max),
    ("enableJumpMove", 2.0, Max),
    ("enableJumpMove_onBattle", 2.0, Max),
];

/// `ImproveFollow`: spirits return to the player beyond ~30 m instead of
/// chasing anywhere (vanilla 9999 m) and may take ladders, holes, navmesh
/// walls and ledges to keep up (lava is left out on purpose).
const FOLLOW: &[(&str, f64, Mode)] = &[
    ("maxBackhomeDist", 30.0, Min),
    ("backhomeDist", 30.0, Min),
    ("backhomeBattleDist", 60.0, Max),
    ("BackHome_LookTargetTime", 15.0, Max),
    ("BackHome_LookTargetDist", 15.0, Max),
    ("BackHomeLifeOnHitEneWal", 0.1, Min),
    ("backToHomeStuckAct", 1.0, Set),
    ("enableNaviFlg_Ladder", 1.0, Max),
    ("enableNaviFlg_Hole", 1.0, Max),
    ("enableNaviFlg_InSideWall", 1.0, Max),
    ("enableNaviFlg_Edge_Ordinary", 1.0, Max),
];

/// The bundles the ini keys switch on, as `(key, entries)`.
const BUNDLES: [(&str, &[(&str, f64, Mode)]); 3] =
    [("ImproveSenses", SENSES), ("ImproveAggression", AGGRESSION), ("ImproveFollow", FOLLOW)];

/// `NpcThinkParam` field table by normalized name.
static THINK_INDEX: LazyLock<HashMap<String, &'static (&'static str, usize, Kind)>> =
    LazyLock::new(|| FIELDS.iter().map(|f| (normalize(f.0), f)).collect());

/// The overrides of the bundles whose ini key is on (default on), as a bit
/// set (for change detection) and the list, in bundle order.
fn bundle_overrides() -> (u8, Vec<Override>) {
    let mut bits = 0u8;
    let mut out = Vec::new();
    for (i, (key, entries)) in BUNDLES.iter().enumerate() {
        if !config::get_bool(key, true) {
            continue;
        }
        bits |= 1 << i;
        for &(name, value, mode) in entries.iter() {
            if let Some(&&(name, offset, kind)) = THINK_INDEX.get(&normalize(name)) {
                out.push(Override { name, offset, kind, value, mode });
            }
        }
    }
    (bits, out)
}

/// Lowercase without `_`, so `isGuard_Act` and `is_guard_act` match.
fn normalize(name: &str) -> String {
    name.chars().filter(|c| *c != '_').map(|c| c.to_ascii_lowercase()).collect()
}

/// The file: `global` lines apply to every spirit row; each `[Row id, id...]`
/// section applies on top of that to the rows with those IDs.
#[derive(Default)]
struct Config {
    global: Vec<Override>,
    rows: Vec<(Vec<u32>, Vec<Override>)>,
    /// `SpiritParam.ini`: applies to the spirits' NpcParam rows.
    npc: Vec<Override>,
}

/// Where the next `Name=value` lines go (in `SpiritThinkParam.ini`).
#[derive(Clone, Copy)]
enum Section {
    Global,
    Row(usize),
}

impl Config {
    fn count(&self) -> usize {
        self.global.len() + self.npc.len() + self.rows.iter().map(|r| r.1.len()).sum::<usize>()
    }

    fn describe(&self) -> String {
        let list = |v: &[Override]| v.iter().map(|o| format!("{}={}", o.name, o.value)).collect::<Vec<_>>().join(", ");
        let mut text = format!("[{}]", list(&self.global));
        if !self.npc.is_empty() {
            text.push_str(&format!(" + NpcParam [{}]", list(&self.npc)));
        }
        for (ids, overrides) in &self.rows {
            text.push_str(&format!(" + {} row id(s): [{}]", ids.len(), list(overrides)));
        }
        text
    }
}

/// `[Row 1, 2, 3]` -> the IDs. `None` if the header is not a Row section.
fn parse_row_header(line: &str) -> Option<Vec<u32>> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?.trim();
    let rest = inner.get(..3).filter(|w| w.eq_ignore_ascii_case("row"))?;
    let ids = inner[rest.len()..].split([',', ' ']).filter(|t| !t.is_empty());
    Some(ids.filter_map(|t| t.parse().ok()).collect())
}

/// Parses the file's text. Bad lines are logged and skipped.
/// `npc` = the content of `SpiritParam.ini` (NpcParam fields, no sections),
/// else `SpiritThinkParam.ini` (NpcThinkParam fields).
fn parse(content: &str, npc: bool) -> Config {
    let think_names = &*THINK_INDEX;
    let npc_names: HashMap<String, &'static (&'static str, usize, Kind)> =
        npc_fields::FIELDS.iter().map(|f| (normalize(f.0), f)).collect();
    let mut config = Config::default();
    let mut section = Section::Global;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.starts_with('[') && npc {
            continue;
        }
        if line.starts_with('[') {
            section = match parse_row_header(line) {
                Some(ids) => {
                    config.rows.push((ids, Vec::new()));
                    Section::Row(config.rows.len() - 1)
                }
                None => Section::Global,
            };
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            logger::error(&format!("ThinkOverride: ignoring line without '=': {line}"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let (names, table) = if npc { (&npc_names, npc_fields::FIELDS) } else { (think_names, FIELDS) };
        let Some(&&(name, offset, kind)) = names.get(&normalize(key)) else {
            let wanted = normalize(key);
            let close: Vec<&str> = table
                .iter()
                .map(|f| f.0)
                .filter(|n| normalize(n).contains(&wanted) || wanted.contains(&normalize(n)))
                .take(5)
                .collect();
            logger::error(&format!(
                "ThinkOverride: unknown field '{key}'{}",
                if close.is_empty() { String::new() } else { format!(", similar: {close:?}") }
            ));
            continue;
        };
        let Ok(value) = value.parse::<f64>() else {
            logger::error(&format!("ThinkOverride: '{key}': '{value}' is not a number."));
            continue;
        };
        let (lo, hi) = match kind {
            Kind::U8 => (0.0, u8::MAX as f64),
            Kind::I8 => (i8::MIN as f64, i8::MAX as f64),
            Kind::U16 => (0.0, u16::MAX as f64),
            Kind::I16 => (i16::MIN as f64, i16::MAX as f64),
            Kind::U32 => (0.0, u32::MAX as f64),
            Kind::I32 => (i32::MIN as f64, i32::MAX as f64),
            Kind::F32 => (f32::MIN as f64, f32::MAX as f64),
            Kind::Bit(_) => (0.0, 1.0),
        };
        if !value.is_finite() || value < lo || value > hi {
            logger::error(&format!("ThinkOverride: '{key}': {value} is out of range ({lo} .. {hi})."));
            continue;
        }
        // The last line for a field wins.
        let list = match section {
            _ if npc => &mut config.npc,
            Section::Row(i) => &mut config.rows[i].1,
            Section::Global => &mut config.global,
        };
        list.retain(|o| o.name != name);
        list.push(Override { name, offset, kind, value, mode: Set });
    }
    config
}

/// Writes `o` into the row at `row`.
///
/// # Safety
/// `row` must point to a whole row of the table `o` came from.
unsafe fn write(row: *mut u8, o: &Override) {
    unsafe {
        let at = row.add(o.offset);
        if o.mode != Set {
            let current = read(at, o.kind);
            if (o.mode == Max && current >= o.value) || (o.mode == Min && current <= o.value) {
                return;
            }
        }
        match o.kind {
            Kind::U8 => at.write(o.value.round() as u8),
            Kind::I8 => at.write(o.value.round() as i8 as u8),
            Kind::U16 => (at as *mut u16).write_unaligned(o.value.round() as u16),
            Kind::I16 => (at as *mut i16).write_unaligned(o.value.round() as i16),
            Kind::U32 => (at as *mut u32).write_unaligned(o.value.round() as u32),
            Kind::I32 => (at as *mut i32).write_unaligned(o.value.round() as i32),
            Kind::F32 => (at as *mut f32).write_unaligned(o.value as f32),
            Kind::Bit(bit) => {
                let byte = at.read() & !(1 << bit);
                at.write(byte | ((o.value.round() as u8) << bit));
            }
        }
    }
}

/// The value of the field of type `kind` at `at`.
///
/// # Safety
/// `at` must point to a field of that type inside a row.
unsafe fn read(at: *const u8, kind: Kind) -> f64 {
    unsafe {
        match kind {
            Kind::U8 => at.read() as f64,
            Kind::I8 => at.read() as i8 as f64,
            Kind::U16 => (at as *const u16).read_unaligned() as f64,
            Kind::I16 => (at as *const i16).read_unaligned() as f64,
            Kind::U32 => (at as *const u32).read_unaligned() as f64,
            Kind::I32 => (at as *const i32).read_unaligned() as f64,
            Kind::F32 => (at as *const f32).read_unaligned() as f64,
            Kind::Bit(bit) => ((at.read() >> bit) & 1) as f64,
        }
    }
}

/// One spirit row: `(row index, row ID, original bytes)`.
type Snapshot = (usize, u32, Vec<u8>);

/// The rows spirits use, per param.
struct Snapshots {
    think: Vec<Snapshot>,
    npc: Vec<Snapshot>,
}

/// Snapshots the `P` rows whose IDs `ids_of` takes from each `BuddyParam`
/// row (`row_size` bytes each). Second value: how many IDs had no row.
fn snapshot_rows<P: SoloParam>(
    repo: &mut SoloParamRepository,
    row_size: usize,
    ids_of: impl Fn(&BUDDY_PARAM_ST) -> [i32; 2],
) -> Option<(Vec<Snapshot>, usize)> {
    let Some(ids) = common::params::row_ids::<P>(repo) else {
        logger::error(&format!("ThinkOverride: {} row IDs could not be read safely.", P::NAME));
        return None;
    };
    let index: HashMap<u32, usize> = ids.iter().copied().enumerate().map(|(i, id)| (id, i)).collect();

    let mut wanted: Vec<usize> = Vec::new();
    let mut missing = 0;
    common::params::for_each_row_mut::<BuddyParam>(repo, |_, row| {
        for id in ids_of(row) {
            if id <= 0 {
                continue;
            }
            match index.get(&(id as u32)) {
                Some(&i) => wanted.push(i),
                None => missing += 1,
            }
        }
    });
    wanted.sort_unstable();
    wanted.dedup();

    let mut rows = Vec::with_capacity(wanted.len());
    for i in wanted {
        let row = repo.get_row_by_index::<P>(i)?;
        let bytes = unsafe { std::slice::from_raw_parts(row as *const _ as *const u8, row_size) };
        rows.push((i, ids[i], bytes.to_vec()));
    }
    Some((rows, missing))
}

/// Every row spirits use, or `None` if the params couldn't be read safely.
fn capture(repo: &mut SoloParamRepository) -> Option<Snapshots> {
    for check in [
        common::params::check::<BuddyParam>(repo),
        common::params::check::<NpcThinkParam>(repo),
        common::params::check::<NpcParam>(repo),
    ] {
        if let Err(err) = check {
            logger::error(&format!("ThinkOverride: {err} - not touching params."));
            return None;
        }
    }
    let (think, missing_think) = snapshot_rows::<NpcThinkParam>(repo, ROW_SIZE, |r| {
        [r.npc_think_param_id(), r.npc_think_param_id_ridden()]
    })?;
    let (npc, missing_npc) =
        snapshot_rows::<NpcParam>(repo, npc_fields::ROW_SIZE, |r| [r.npc_param_id(), r.npc_param_id_ridden()])?;
    logger::log(&format!(
        "ThinkOverride: {} NpcThinkParam row(s) ({missing_think} id(s) without a row) and {} NpcParam row(s) ({missing_npc} without a row) used by spirits.",
        think.len(),
        npc.len()
    ));
    Some(Snapshots { think, npc })
}

/// Puts every `P` row back from its snapshot, then lets `write_over` write
/// onto it (given the row ID).
fn restore<P: SoloParam>(
    repo: &mut SoloParamRepository,
    rows: &[Snapshot],
    row_size: usize,
    write_over: impl Fn(*mut u8, u32),
) {
    for (i, id, original) in rows {
        let Some(row) = repo.get_row_by_index_mut::<P>(*i) else {
            continue;
        };
        let ptr = row as *mut _ as *mut u8;
        unsafe { std::ptr::copy_nonoverlapping(original.as_ptr(), ptr, row_size) };
        write_over(ptr, *id);
    }
}

/// Restores every row from its snapshot, then writes the overrides: the
/// bundles, the global and `[Row ...]` ones onto the NpcThinkParam rows, `[NpcParam]` ones onto
/// the NpcParam rows.
fn apply(repo: &mut SoloParamRepository, rows: &Snapshots, bundles: &[Override], config: &Config) {
    restore::<NpcThinkParam>(repo, &rows.think, ROW_SIZE, |ptr, id| unsafe {
        for o in bundles.iter().chain(&config.global) {
            write(ptr, o);
        }
        for (_, overrides) in config.rows.iter().filter(|(ids, _)| ids.contains(&id)) {
            for o in overrides {
                write(ptr, o);
            }
        }
    });
    restore::<NpcParam>(repo, &rows.npc, npc_fields::ROW_SIZE, |ptr, _| unsafe {
        for o in &config.npc {
            write(ptr, o);
        }
    });
}

pub fn run(dir: String) {
    let think_path = format!("{dir}\\{THINK_FILE}");
    let npc_path = format!("{dir}\\{NPC_FILE}");
    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    // None = not captured yet; Some(None) = capture failed, stay off.
    let mut rows: Option<Option<Snapshots>> = None;
    // What was last applied: the files' modified times (None = absent) and the
    // set of bundles on.
    let mut applied: Option<(Option<SystemTime>, Option<SystemTime>, u8)> = None;

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "ThinkOverride",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            let modified_of = |path: &str| std::fs::metadata(path).and_then(|m| m.modified()).ok();
            let (bits, bundles) = bundle_overrides();
            let signature = (modified_of(&think_path), modified_of(&npc_path), bits);
            if applied == Some(signature) {
                return;
            }
            // Nothing wanted and nothing applied before: leave the params alone.
            if applied.is_none() && signature == (None, None, 0) {
                applied = Some(signature);
                return;
            }
            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }
            let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
                return;
            };
            let Some(rows) = rows.get_or_insert_with(|| capture(repo)) else {
                return;
            };

            // A missing file = no overrides; one locked while being saved:
            // try again next tick.
            let read = |path: &str, exists: bool, npc: bool| -> Option<Config> {
                if !exists {
                    return Some(Config::default());
                }
                std::fs::read_to_string(path).ok().map(|content| parse(&content, npc))
            };
            let (Some(mut config), Some(npc_config)) = (
                read(&think_path, signature.0.is_some(), false),
                read(&npc_path, signature.1.is_some(), true),
            ) else {
                return;
            };
            config.npc = npc_config.npc;
            apply(repo, rows, &bundles, &config);
            let names: Vec<&str> = BUNDLES.iter().enumerate().filter(|(i, _)| bits & (1 << i) != 0).map(|(_, b)| b.0).collect();
            logger::log(&format!(
                "ThinkOverride: {names:?} + {} file field(s) {} written to {} row(s). Re-summon spirits for it to take effect.",
                config.count(),
                config.describe(),
                rows.think.len() + rows.npc.len()
            ));
            applied = Some(signature);
        },
    );

    logger::log("ThinkOverride: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str) -> (usize, Kind) {
        let f = FIELDS.iter().find(|f| f.0 == name).unwrap();
        (f.1, f.2)
    }

    /// The generated offsets must match the real struct: set a field through
    /// the crate's own setter and read it back at the table's offset.
    #[test]
    fn offsets_match_struct() {
        let mut row: NPC_THINK_PARAM_ST = unsafe { std::mem::zeroed() };
        row.set_team_attack_effectivity(77);
        row.set_battle_start_dist(1234);
        row.set_is_guard_act(9);
        row.set_memory_target_forget_time(2.5);
        row.set_logic_id(-5);
        row.set_surprise_anim_id(31337);
        let base = &row as *const _ as *const u8;
        let at = |name: &str| unsafe { base.add(field(name).0) };
        unsafe {
            assert_eq!(at("team_attack_effectivity").read(), 77);
            assert_eq!((at("battle_start_dist") as *const u16).read_unaligned(), 1234);
            assert_eq!(at("is_guard_act").read(), 9);
            assert_eq!((at("memory_target_forget_time") as *const f32).read_unaligned(), 2.5);
            assert_eq!((at("logic_id") as *const i32).read_unaligned(), -5);
            assert_eq!((at("surprise_anim_id") as *const i32).read_unaligned(), 31337);
        }
    }

    fn write_all(row: *mut u8, overrides: &[Override]) {
        for o in overrides {
            unsafe { write(row, o) };
        }
    }

    #[test]
    fn write_roundtrip_and_names() {
        let parsed = parse(
            "; c\nTeamAttackEffectivity = 100\nisGuard_Act=1\nbogus=3\nBattleStartDist=70000\nsearchEye_dist=15",
            false,
        );
        let names: Vec<_> = parsed.global.iter().map(|o| o.name).collect();
        assert_eq!(names, ["team_attack_effectivity", "is_guard_act", "search_eye_dist"]);
        let mut row: NPC_THINK_PARAM_ST = unsafe { std::mem::zeroed() };
        write_all(&mut row as *mut _ as *mut u8, &parsed.global);
        assert_eq!(row.team_attack_effectivity(), 100);
        assert_eq!(row.is_guard_act(), 1);
        assert_eq!(row.search_eye_dist(), 15);
    }

    /// `SpiritParam.ini`: NpcParam names; section headers are ignored.
    #[test]
    fn npc_file() {
        let parsed = parse("[Whatever]\nfallDamageDump=100\nhp=5\nbogusfield=1\neye_dist=7", true);
        assert!(parsed.global.is_empty());
        let names: Vec<_> = parsed.npc.iter().map(|o| o.name).collect();
        assert_eq!(names, ["fall_damage_dump", "hp"]);
        let mut row: NPC_PARAM_ST = unsafe { std::mem::zeroed() };
        write_all(&mut row as *mut _ as *mut u8, &parsed.npc);
        assert_eq!(row.fall_damage_dump(), 100);
        assert_eq!(row.hp(), 5);
    }

    /// Every bundle entry names a real field, and its value fits the type.
    #[test]
    fn bundles_resolve() {
        for (key, entries) in BUNDLES {
            for &(name, value, _) in entries {
                let Some(&&(_, _, kind)) = THINK_INDEX.get(&normalize(name)) else {
                    panic!("{key}: unknown field {name}");
                };
                let max = match kind {
                    Kind::U8 => 255.0,
                    Kind::U16 => 65535.0,
                    Kind::Bit(_) => 1.0,
                    _ => f64::MAX,
                };
                assert!(value >= 0.0 && value <= max, "{key}: {name}={value} out of range");
            }
        }
    }

    /// `Max` / `Min` only move a value in their direction; `Set` always writes.
    #[test]
    fn modes() {
        let mut row: NPC_THINK_PARAM_ST = unsafe { std::mem::zeroed() };
        let ptr = &mut row as *mut _ as *mut u8;
        let o = |name: &'static str, value: f64, mode: Mode| {
            let &&(name, offset, kind) = THINK_INDEX.get(&normalize(name)).unwrap();
            Override { name, offset, kind, value, mode }
        };
        row.set_battle_start_dist(40);
        row.set_max_backhome_dist(9999);
        row.set_team_attack_effectivity(30);
        row.set_enable_navi_flg_ladder(true);
        unsafe {
            write(ptr, &o("BattleStartDist", 15.0, Max)); // 40 stays
            write(ptr, &o("maxBackhomeDist", 30.0, Min)); // 9999 -> 30
            write(ptr, &o("TeamAttackEffectivity", 100.0, Max)); // 30 -> 100
            write(ptr, &o("enableNaviFlg_Ladder", 1.0, Max)); // stays 1
            write(ptr, &o("enableNaviFlg_Hole", 1.0, Max)); // 0 -> 1
            write(ptr, &o("nose_dist", 7.0, Set));
        }
        assert_eq!(row.battle_start_dist(), 40);
        assert_eq!(row.max_backhome_dist(), 30);
        assert_eq!(row.team_attack_effectivity(), 100);
        assert!(row.enable_navi_flg_ladder() && row.enable_navi_flg_hole());
        assert_eq!(row.nose_dist(), 7);
    }

    #[test]
    fn row_sections() {
        let parsed = parse(
            "goalAction_ToCaution=3\n[Row 143002000, 141800000]\ngoalAction_ToCaution=2\nBattleStartDist=9\n[All]\neye_dist=7\n",
            false,
        );
        assert_eq!(parsed.global.len(), 2);
        assert_eq!(parsed.rows.len(), 1);
        assert_eq!(parsed.rows[0].0, [143002000, 141800000]);
        assert_eq!(parsed.rows[0].1.len(), 2);
    }

    /// Bit flags: only the named bit changes, neighbours are kept.
    #[test]
    fn bit_flags() {
        let mut row: NPC_THINK_PARAM_ST = unsafe { std::mem::zeroed() };
        row.set_enable_navi_flg_edge(true);
        row.set_enable_navi_flg_door(true);
        let parsed = parse(
            "enableNaviFlg_Lava=1\nenableNaviFlg_Edge_Ordinary=1\nenableNaviFlg_Door=0\nenableNaviFlg_Ladder=2",
            false,
        );
        assert_eq!(parsed.global.len(), 3);
        write_all(&mut row as *mut _ as *mut u8, &parsed.global);
        assert!(row.enable_navi_flg_edge() && row.enable_navi_flg_lava() && row.enable_navi_flg_edge_ordinary());
        assert!(!row.enable_navi_flg_door() && !row.enable_navi_flg_ladder());
    }
}
