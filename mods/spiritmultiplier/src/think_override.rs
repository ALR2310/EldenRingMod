//! `SpiritThinkParam.ini` and `SpiritParam.ini` (optional, experimental, hot
//! reload without F5): set any scalar field / bit flag of the rows spirits use
//! - the `NpcThinkParam` ones (the AI: aggressiveness, sight, battle start
//! distance, ...) and the `NpcParam` ones - so tweaks can be tried in-game
//! without rebuilding the DLL.
//!
//! The files live next to the DLL and are only read if they exist. One
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
use std::time::{Duration, SystemTime};

use eldenring::cs::{BuddyParam, CSTaskGroupIndex, NpcParam, NpcThinkParam, SoloParam, SoloParamRepository};
use eldenring::param::{BUDDY_PARAM_ST, NPC_PARAM_ST, NPC_THINK_PARAM_ST};
use fromsoftware_shared::FromStatic;

use common::logger;

mod think_fields {
    include!("think_fields.rs");
}
mod npc_fields {
    include!("npc_fields.rs");
}
use think_fields::{FIELDS, ROW_SIZE};

type Table = &'static [(&'static str, usize, Kind)];

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

/// One validated line of the file: `(field name, byte offset, type, value)`.
struct Override {
    name: &'static str,
    offset: usize,
    kind: Kind,
    value: f64,
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
    let by_name = |table: Table| -> HashMap<String, &'static (&'static str, usize, Kind)> {
        table.iter().map(|f| (normalize(f.0), f)).collect()
    };
    let think_names = by_name(FIELDS);
    let npc_names = by_name(npc_fields::FIELDS);
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
        let (names, table) = if npc { (&npc_names, npc_fields::FIELDS) } else { (&think_names, FIELDS) };
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
        list.push(Override { name, offset, kind, value });
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

/// Restores every row from its snapshot, then writes the overrides: global
/// and `[Row ...]` ones onto the NpcThinkParam rows, `[NpcParam]` ones onto
/// the NpcParam rows.
fn apply(repo: &mut SoloParamRepository, rows: &Snapshots, config: &Config) {
    restore::<NpcThinkParam>(repo, &rows.think, ROW_SIZE, |ptr, id| unsafe {
        for o in &config.global {
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
    // Modified time of the file when last applied (None = file absent).
    let mut applied: Option<(Option<SystemTime>, Option<SystemTime>)> = None;

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
            let modified = (modified_of(&think_path), modified_of(&npc_path));
            // Nothing to do (and nothing touched) until a file first exists.
            if applied.is_none() && modified == (None, None) {
                return;
            }
            if applied == Some(modified) {
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
                read(&think_path, modified.0.is_some(), false),
                read(&npc_path, modified.1.is_some(), true),
            ) else {
                return;
            };
            config.npc = npc_config.npc;
            apply(repo, rows, &config);
            logger::log(&format!(
                "ThinkOverride: {} field(s) {} written to {} row(s). Re-summon spirits for it to take effect.",
                config.count(),
                config.describe(),
                rows.think.len() + rows.npc.len()
            ));
            applied = Some(modified);
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
