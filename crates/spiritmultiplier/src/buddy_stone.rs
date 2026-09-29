//! Spirit summoning pools (`BuddyStoneParam` - one row per summoning pool
//! area), two hot-reload toggles:
//!
//! - `NoRestResummon`: re-summon after the spirits die (or are sent back)
//!   without resting at a Site of Grace. The once-per-rest lock is the
//!   row's `summonedEventFlagId` (+0xC): the game raises that event flag on
//!   a summon and refuses the pool while it is on; resting clears it.
//!   Written as 0 = no flag, so nothing ever locks the pool.
//! - `SummonAnywhere`: summon outside the vanilla areas. Every row gets
//!   `activateRange` (+0x1C) = 65535, `overwriteReturnRange` (+0x1E) = -1
//!   (no override), `overwriteActivateRegionEntityId` (+0x20) = 0 (use the
//!   range, not a map region) and `eliminateTargetEntityId` (+0x8) = 0 (the
//!   pool no longer closes once its boss is dead).
//!
//! Both are exactly what Solid Uncapper 2.3.3 writes for its `No-Rest
//! Re-Summon` / `Summon Anywhere` settings (`sub_180031EC0` /
//! `sub_1800321B0`, snapshot in `sub_180031D30` - see README 2026-09-29);
//! re-implemented here. Rows are walked by index (`common::params`), and
//! the original values are captured once before the first write, so every
//! later change is applied from that snapshot (F5 works both ways).

use std::time::Duration;

use eldenring::cs::{BuddyStoneParam, CSTaskGroupIndex, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use common::{config, logger};

const TICK_INTERVAL_MS: f64 = 1000.0;

const ANYWHERE_ACTIVATE_RANGE: u16 = u16::MAX;
const ANYWHERE_RETURN_RANGE: i16 = -1;

/// Original values of the fields this module writes, per row index.
struct Original {
    index: usize,
    eliminate_target_entity_id: u32,
    summoned_event_flag_id: u32,
    activate_range: u16,
    overwrite_return_range: i16,
    overwrite_activate_region_entity_id: u32,
}

fn capture(repo: &mut SoloParamRepository) -> Option<Vec<Original>> {
    if let Err(err) = common::params::check::<BuddyStoneParam>(repo) {
        logger::error(&format!("BuddyStone: {err} - not touching params."));
        return None;
    }
    let mut rows = Vec::new();
    common::params::for_each_row_mut::<BuddyStoneParam>(repo, |index, row| {
        rows.push(Original {
            index,
            eliminate_target_entity_id: row.eliminate_target_entity_id(),
            summoned_event_flag_id: row.summoned_event_flag_id(),
            activate_range: row.activate_range(),
            overwrite_return_range: row.overwrite_return_range(),
            overwrite_activate_region_entity_id: row.overwrite_activate_region_entity_id(),
        });
    });
    let locked = rows.iter().filter(|r| r.summoned_event_flag_id != 0).count();
    logger::log(&format!(
        "BuddyStone: {} summoning pool row(s), {locked} with a once-per-rest flag.",
        rows.len()
    ));
    Some(rows)
}

pub fn run() {
    let cs_task = common::task::wait_for_cs_task();

    let mut elapsed_ms: f64 = 0.0;
    // None = not captured yet; Some(None) = capture failed, stay off.
    let mut originals: Option<Option<Vec<Original>>> = None;
    let mut applied: Option<(bool, bool)> = None;

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "BuddyStone",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            elapsed_ms += (data.delta_time.time as f64) * 1000.0;
            if elapsed_ms < TICK_INTERVAL_MS {
                return;
            }
            elapsed_ms = 0.0;

            // Same in-world gate as every other param-touching feature.
            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }
            let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
                return;
            };

            let no_rest = config::get_bool("NoRestResummon", true);
            let anywhere = config::get_bool("SummonAnywhere", true);
            let Some(rows) = originals.get_or_insert_with(|| capture(repo)) else {
                return;
            };
            if applied == Some((no_rest, anywhere)) {
                return;
            }

            for original in rows.iter() {
                let Some(row) = repo.get_row_by_index_mut::<BuddyStoneParam>(original.index) else {
                    continue;
                };
                row.set_summoned_event_flag_id(if no_rest { 0 } else { original.summoned_event_flag_id });
                if anywhere {
                    row.set_activate_range(ANYWHERE_ACTIVATE_RANGE);
                    row.set_overwrite_return_range(ANYWHERE_RETURN_RANGE);
                    row.set_overwrite_activate_region_entity_id(0);
                    row.set_eliminate_target_entity_id(0);
                } else {
                    row.set_activate_range(original.activate_range);
                    row.set_overwrite_return_range(original.overwrite_return_range);
                    row.set_overwrite_activate_region_entity_id(original.overwrite_activate_region_entity_id);
                    row.set_eliminate_target_entity_id(original.eliminate_target_entity_id);
                }
            }
            logger::log(&format!(
                "NoRestResummon={no_rest}, SummonAnywhere={anywhere}: applied to {} pool row(s).",
                rows.len()
            ));
            applied = Some((no_rest, anywhere));
        },
    );

    logger::log("BuddyStone: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
