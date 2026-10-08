//! `SpiritThink.ini` (optional, experimental, hot reload without F5): sets
//! any scalar field of the `NpcThinkParam` rows spirits use (the AI
//! parameters: aggressiveness, sight, battle start distance, ...), so AI
//! tweaks can be tried in-game without rebuilding the DLL.
//!
//! The file lives next to the DLL and is only read if it exists. One
//! `Name=value` per line, `;`/`#` comments, sections ignored. `Name` is the
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

use eldenring::cs::{BuddyParam, CSTaskGroupIndex, NpcThinkParam, SoloParamRepository};
use eldenring::param::NPC_THINK_PARAM_ST;
use fromsoftware_shared::FromStatic;

use common::logger;

mod think_fields {
    include!("think_fields.rs");
}
use think_fields::{FIELDS, ROW_SIZE};

const FILE_NAME: &str = "SpiritThink.ini";
const TICK_INTERVAL_MS: f64 = 1000.0;

const _: () = assert!(size_of::<NPC_THINK_PARAM_ST>() == ROW_SIZE);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    U8,
    U16,
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

/// Parses the file's text. Bad lines are logged and skipped.
fn parse(content: &str) -> Vec<Override> {
    let by_name: HashMap<String, &(&'static str, usize, Kind)> =
        FIELDS.iter().map(|f| (normalize(f.0), f)).collect();
    let mut out: Vec<Override> = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';', '[']) {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            logger::error(&format!("ThinkOverride: ignoring line without '=': {line}"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let Some(&&(name, offset, kind)) = by_name.get(&normalize(key)) else {
            let wanted = normalize(key);
            let close: Vec<&str> = FIELDS
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
            Kind::U16 => (0.0, u16::MAX as f64),
            Kind::I32 => (i32::MIN as f64, i32::MAX as f64),
            Kind::F32 => (f32::MIN as f64, f32::MAX as f64),
            Kind::Bit(_) => (0.0, 1.0),
        };
        if !value.is_finite() || value < lo || value > hi {
            logger::error(&format!("ThinkOverride: '{key}': {value} is out of range ({lo} .. {hi})."));
            continue;
        }
        // The last line for a field wins.
        out.retain(|o| o.name != name);
        out.push(Override { name, offset, kind, value });
    }
    out
}

/// Writes `o` into the row at `row`.
///
/// # Safety
/// `row` must point to a whole `NPC_THINK_PARAM_ST` (`ROW_SIZE` bytes).
unsafe fn write(row: *mut u8, o: &Override) {
    debug_assert!(o.offset + 4 <= ROW_SIZE || o.kind != Kind::I32);
    unsafe {
        let at = row.add(o.offset);
        match o.kind {
            Kind::U8 => at.write(o.value.round() as u8),
            Kind::U16 => (at as *mut u16).write_unaligned(o.value.round() as u16),
            Kind::I32 => (at as *mut i32).write_unaligned(o.value.round() as i32),
            Kind::F32 => (at as *mut f32).write_unaligned(o.value as f32),
            Kind::Bit(bit) => {
                let byte = at.read() & !(1 << bit);
                at.write(byte | ((o.value.round() as u8) << bit));
            }
        }
    }
}

/// `(NpcThinkParam row index, original bytes)` for every row spirits use, or
/// `None` if the params couldn't be read safely.
fn capture(repo: &mut SoloParamRepository) -> Option<Vec<(usize, Vec<u8>)>> {
    for check in [common::params::check::<BuddyParam>(repo), common::params::check::<NpcThinkParam>(repo)] {
        if let Err(err) = check {
            logger::error(&format!("ThinkOverride: {err} - not touching params."));
            return None;
        }
    }
    let Some(ids) = common::params::row_ids::<NpcThinkParam>(repo) else {
        logger::error("ThinkOverride: param row IDs could not be read safely - not touching params.");
        return None;
    };
    let index: HashMap<u32, usize> = ids.into_iter().enumerate().map(|(i, id)| (id, i)).collect();

    let mut wanted: Vec<usize> = Vec::new();
    let mut missing = 0;
    common::params::for_each_row_mut::<BuddyParam>(repo, |_, row| {
        for id in [row.npc_think_param_id(), row.npc_think_param_id_ridden()] {
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
        let row = repo.get_row_by_index::<NpcThinkParam>(i)?;
        let bytes = unsafe { std::slice::from_raw_parts(row as *const _ as *const u8, ROW_SIZE) };
        rows.push((i, bytes.to_vec()));
    }
    logger::log(&format!(
        "ThinkOverride: {} NpcThinkParam row(s) used by spirits ({missing} BuddyParam id(s) without a row).",
        rows.len()
    ));
    Some(rows)
}

/// Restores every row from its snapshot, then writes `overrides`.
fn apply(repo: &mut SoloParamRepository, rows: &[(usize, Vec<u8>)], overrides: &[Override]) {
    for (i, original) in rows {
        let Some(row) = repo.get_row_by_index_mut::<NpcThinkParam>(*i) else {
            continue;
        };
        let ptr = row as *mut _ as *mut u8;
        unsafe {
            std::ptr::copy_nonoverlapping(original.as_ptr(), ptr, ROW_SIZE);
            for o in overrides {
                write(ptr, o);
            }
        }
    }
}

pub fn run(dir: String) {
    let path = format!("{dir}\\{FILE_NAME}");
    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    // None = not captured yet; Some(None) = capture failed, stay off.
    let mut rows: Option<Option<Vec<(usize, Vec<u8>)>>> = None;
    // Modified time of the file when last applied (None = file absent).
    let mut applied: Option<Option<SystemTime>> = None;

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

            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            // Nothing to do (and nothing touched) until the file first exists.
            if applied.is_none() && modified.is_none() {
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

            let overrides = match modified {
                Some(_) => match std::fs::read_to_string(&path) {
                    Ok(content) => parse(&content),
                    // Locked while being saved: try again next tick.
                    Err(_) => return,
                },
                None => Vec::new(),
            };
            apply(repo, rows, &overrides);
            let list: Vec<String> = overrides.iter().map(|o| format!("{}={}", o.name, o.value)).collect();
            logger::log(&format!(
                "ThinkOverride: {} field(s) {list:?} written to {} row(s). Re-summon spirits for it to take effect.",
                overrides.len(),
                rows.len()
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

    #[test]
    fn write_roundtrip_and_names() {
        let parsed = parse("; c\nTeamAttackEffectivity = 100\nisGuard_Act=1\nbogus=3\nBattleStartDist=70000\nsearchEye_dist=15");
        let names: Vec<_> = parsed.iter().map(|o| o.name).collect();
        assert_eq!(names, ["team_attack_effectivity", "is_guard_act", "search_eye_dist"]);
        let mut row: NPC_THINK_PARAM_ST = unsafe { std::mem::zeroed() };
        for o in &parsed {
            unsafe { write(&mut row as *mut _ as *mut u8, o) };
        }
        assert_eq!(row.team_attack_effectivity(), 100);
        assert_eq!(row.is_guard_act(), 1);
        assert_eq!(row.search_eye_dist(), 15);
    }

    /// Bit flags: only the named bit changes, neighbours are kept.
    #[test]
    fn bit_flags() {
        let mut row: NPC_THINK_PARAM_ST = unsafe { std::mem::zeroed() };
        row.set_enable_navi_flg_edge(true);
        row.set_enable_navi_flg_door(true);
        let parsed = parse("enableNaviFlg_Lava=1
enableNaviFlg_Edge_Ordinary=1
enableNaviFlg_Door=0
enableNaviFlg_Ladder=2");
        assert_eq!(parsed.len(), 3);
        for o in &parsed {
            unsafe { write(&mut row as *mut _ as *mut u8, o) };
        }
        assert!(row.enable_navi_flg_edge() && row.enable_navi_flg_lava() && row.enable_navi_flg_edge_ordinary());
        assert!(!row.enable_navi_flg_door() && !row.enable_navi_flg_ladder());
    }
}
