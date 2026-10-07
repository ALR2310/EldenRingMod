//! Makes Scarlet Rot's and Poison's damage-over-time ticks last for a
//! configurable duration (seconds) instead of their vanilla 90s, by
//! editing every `SpEffectParam` row that matches a specific field
//! signature - confirmed live, in-game, against the running
//! `SoloParamRepository` (not just a static CSV export).
//!
//! ## How the row set was found (Scarlet Rot)
//!
//! Earlier attempts assumed a single shared row (`505`, "Poison/Scarlet
//! Rot (Cycled)") controlled the DoT's duration - dropped once static CSV
//! analysis couldn't be trusted (see git history of this file). Instead,
//! the actual running game's `SpEffectParam` table was queried directly
//! and progressively narrowed down by adding one field condition at a
//! time, logging the matching row count and a wide field snapshot after
//! each addition, until the exact intended row set was isolated:
//!
//! - `SpCategory == 10005` and `StateInfo == 5`: Scarlet Rot's own
//!   category.
//! - `ChangeHpRate != 0`: only rows that actually drain HP over time
//!   (excludes pure buildup-boost/talisman rows).
//! - `EffectEndurance == 90`: the vanilla duration itself, which is also
//!   what separates the rows the *player inflicts on enemies* (this
//!   module's actual target) from unrelated rows that happened to share
//!   the first 3 conditions - confirmed by cross-referencing the matched
//!   IDs against SmithBox by hand:
//!   - `EffectEndurance=180`: environmental "Scarlet Rot Swamp" hazards
//!     (`TargetEnemy=false`, unlike every other match) and "NPC: Scarlet
//!     Rot" rows (an NPC-only AI-driven variant, unrelated to what the
//!     player's own weapons/spells inflict).
//!   - `EffectEndurance=300`: Malenia's own unique rot effect - a boss
//!     special case, not something to touch here.
//!   - `EffectEndurance=30`: 2 unnamed rows with no `CycleOccurrenceSpEffectId`
//!     (i.e. they don't feed back into the DoT cycle at all - structurally
//!     unrelated one-shots).
//!   - Excluding all of the above by requiring `EffectEndurance == 90`
//!     leaves exactly the "Type 1/Special N", "Low/Medium/High -
//!     Addition/Innate +0..+25" families, plus a handful of item/talisman/
//!     incantation rows that share the same signature - all genuinely
//!     player-inflicted Scarlet Rot.
//!
//! Poison (`SpCategory == 10004`) mirrors Scarlet Rot's own family names
//! row for row ("Type 1/Special N", "Low/Medium/High -
//! Addition/Innate/Special +N") and the same `EffectEndurance == 90`
//! outliers to exclude ("Type 2 - Special N" at `30`, "Poison Swamp"/"NPC:
//! Poison" at `180`, "NPC: Fast Poison" at `30`) - but **not** the same
//! `StateInfo`: assuming `StateInfo == 5` (Scarlet Rot's own value) for
//! Poison too matched 0 rows live in-game, since Poison actually uses
//! `StateInfo == 2` (confirmed by the user reading it directly in
//! SmithBox) - each ailment's own `state_info` is a separate field on
//! [Ailment] rather than a shared constant, precisely because it isn't
//! shared.
//!
//! Each ailment's `PercentDamage`/`FixedDamage` ini defaults below
//! (`ChangeHpRate`/`ChangeHpPoint`) are read from its own vanilla rows
//! directly in this same exported data, not guessed - a previous default
//! guess for Scarlet Rot's `FixedDamage` (assumed `0`) turned out wrong
//! (real vanilla is `15`, confirmed by the user reading it themselves in
//! SmithBox), so trust what's actually in the CSV/live data over
//! assumption.
//!
//! Supports `General.ReloadKey` hot reload via [`common::reload`], so
//! settings can be tweaked and re-applied without restarting the game -
//! each ailment's row IDs are snapshotted once on its first successful
//! match (while `EffectEndurance` still holds its vanilla value) and
//! reused on every later reload, rather than re-running the same
//! field-matching query against the live table. That query includes
//! `EffectEndurance == 90` itself, so once a row's been edited to
//! something other than `90` it would silently stop matching its own
//! query on the very next reload - looking like the edit "didn't take"
//! even though it did, just not reapplied. Re-scanning is only needed the
//! one time per ailment.

use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration as StdDuration;

use eldenring::cs::{CSTaskGroupIndex, SoloParamRepository, SpEffectParam};
use eldenring::param::SP_EFFECT_PARAM_ST;
use fromsoftware_shared::FromStatic;

use common::config;
use common::logger;

/// The vanilla duration (seconds) shared by both ailments' targeted rows -
/// also part of the filter signature itself (see the module doc comment
/// for why `EffectEndurance == 90` is required, not just `!= 0`).
const VANILLA_DURATION: f32 = 90.0;

/// One status ailment's own `SpCategory`/`StateInfo` values, ini key
/// prefix, and vanilla `ChangeHpRate`/`ChangeHpPoint` defaults - see the
/// module doc comment for where each of these came from. `StateInfo`
/// differs per ailment (`5` for Scarlet Rot, `2` for Poison) - confirmed
/// live by the user reading it in SmithBox after the shared hardcoded `5`
/// matched 0 Poison rows.
struct Ailment {
    name: &'static str,
    sp_category: u16,
    state_info: u16,
    ini_prefix: &'static str,
    default_percent_damage: f64,
    default_fixed_damage: i32,
    matched_row_ids: Mutex<Option<Vec<u32>>>,
}

static SCARLET_ROT: Ailment = Ailment {
    name: "Scarlet Rot",
    sp_category: 10005,
    state_info: 5,
    ini_prefix: "ScarletRot",
    default_percent_damage: 0.18,
    default_fixed_damage: 15,
    matched_row_ids: Mutex::new(None),
};

static POISON: Ailment = Ailment {
    name: "Poison",
    sp_category: 10004,
    state_info: 2,
    ini_prefix: "Poison",
    default_percent_damage: 0.07,
    default_fixed_damage: 7,
    matched_row_ids: Mutex::new(None),
};

/// The 3 user-configurable knobs applied to every row targeted by one
/// [Ailment], read fresh on every [apply] call (startup and every reload).
struct Settings {
    duration: f32,
    percent_damage: f32,
    fixed_damage: i32,
}

fn build_settings(ailment: &Ailment) -> Settings {
    Settings {
        duration: config::get_double(&format!("{}.Duration", ailment.ini_prefix), -1.0) as f32,
        percent_damage: config::get_double(
            &format!("{}.PercentDamage", ailment.ini_prefix),
            ailment.default_percent_damage,
        ) as f32,
        fixed_damage: config::get_int(
            &format!("{}.FixedDamage", ailment.ini_prefix),
            ailment.default_fixed_damage,
        ),
    }
}

/// Whether `row` is one of the player-inflicted rows `ailment` targets -
/// see the module doc comment for how this exact signature was confirmed
/// against the live game. Only ever evaluated against a row still holding
/// its vanilla `EffectEndurance` (each ailment's first scan) - see
/// [Ailment::matched_row_ids].
fn matches(ailment: &Ailment, row: &SP_EFFECT_PARAM_ST) -> bool {
    row.effect_endurance() == VANILLA_DURATION
        && row.sp_category() == ailment.sp_category
        && row.state_info() == ailment.state_info
        && row.change_hp_rate() != 0.0
}

/// Sets `EffectEndurance`, `ChangeHpRate`, and `ChangeHpPoint` from
/// `settings` on every row `ailment` targets - matched by field signature
/// on the first call for this ailment (caching the IDs found), and by the
/// cached IDs directly on every later call. Returns the IDs touched, for
/// logging.
fn apply(ailment: &Ailment, repo: &mut SoloParamRepository, settings: &Settings) -> Vec<u32> {
    let mut cache = ailment.matched_row_ids.lock().unwrap();
    let ids = cache.get_or_insert_with(|| {
        repo.rows::<SpEffectParam>()
            .filter(|(_, row)| matches(ailment, row))
            .map(|(id, _)| id)
            .collect()
    });

    let mut touched = Vec::new();
    for &id in ids.iter() {
        if let Some(row) = repo.get_mut::<SpEffectParam>(id) {
            row.set_effect_endurance(settings.duration);
            row.set_change_hp_rate(settings.percent_damage);
            row.set_change_hp_point(settings.fixed_damage);
            touched.push(id);
        }
    }
    touched
}

fn log_result(ailment: &Ailment, settings: &Settings, touched: &[u32], suffix: &str) {
    logger::log(&format!(
        "SpEffectParam: EffectEndurance={}, ChangeHpRate={}, ChangeHpPoint={} set on {} {} \
         row(s){suffix}: {touched:?}",
        settings.duration,
        settings.percent_damage,
        settings.fixed_damage,
        touched.len(),
        ailment.name,
    ));
}

fn apply_and_log(ailment: &'static Ailment, repo: &mut SoloParamRepository, suffix: &str) {
    let settings = build_settings(ailment);
    let touched = apply(ailment, repo, &settings);
    log_result(ailment, &settings, &touched, suffix);
}

/// Verification-only: logs every field of every row `ailment` matched (as
/// currently cached in [Ailment::matched_row_ids]), one line per row, so
/// the found row set can be checked by eye against SmithBox instead of
/// just trusting the ID list in [log_result]. Meant to be called once,
/// right after the first [apply_and_log] - not on every reload.
fn log_matched_rows_detail(ailment: &Ailment, repo: &SoloParamRepository) {
    let cache = ailment.matched_row_ids.lock().unwrap();
    let Some(ids) = cache.as_ref() else {
        logger::warn(&format!(
            "{}: no matched rows cached yet - call after apply_and_log.",
            ailment.name
        ));
        return;
    };

    logger::log(&format!(
        "{}: detailed dump of {} matched row(s):",
        ailment.name,
        ids.len()
    ));
    for &id in ids {
        let Some(row) = repo.get::<SpEffectParam>(id) else {
            logger::warn(&format!("  - ID {id}: not found (unexpected)."));
            continue;
        };
        logger::log(&format!(
            "  - ID {id}: EffectEndurance={}, ChangeHpRate={}, ChangeHpPoint={}, \
             PoizonAtkPower={}, DiseaseAtkPower={}, TargetEnemy={}, TargetPlayer={}, \
             CycleOccSpEffectId={}",
            row.effect_endurance(),
            row.change_hp_rate(),
            row.change_hp_point(),
            row.poizon_attack_power(),
            row.disease_attack_power(),
            row.effect_target_enemy(),
            row.effect_target_player(),
            row.cycle_occurrence_sp_effect_id(),
        ));
    }
}

/// Applies both ailments' settings once, waiting (forever) for
/// `SoloParamRepository` to actually be populated, then watches
/// `General.ReloadKey` (via [`common::reload`]) on the game's own
/// `FrameBegin` task group for the rest of the DLL's lifetime, re-reading
/// and re-applying both on every press. Meant to run on its own worker
/// thread spawned from `DllMain`; never returns.
pub fn run() {
    let mut last_seen_generation = common::reload::RELOAD_GENERATION.load(Ordering::Relaxed);

    let repo = common::player::wait_for_solo_param_repository();
    apply_and_log(&SCARLET_ROT, repo, "");
    apply_and_log(&POISON, repo, "");
    log_matched_rows_detail(&POISON, repo);

    let cs_task = common::task::wait_for_cs_task();
    common::task::run_recurring_safe(
        cs_task,
        "InfiniteAilment",
        CSTaskGroupIndex::FrameBegin,
        move |_data: &eldenring::fd4::FD4TaskData| {
            let generation = common::reload::RELOAD_GENERATION.load(Ordering::Relaxed);
            if generation == last_seen_generation {
                return;
            }
            last_seen_generation = generation;

            let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else {
                logger::warn("SoloParamRepository not available on reload, skipped.");
                return;
            };
            apply_and_log(&SCARLET_ROT, repo, " (hotkey pressed)");
            apply_and_log(&POISON, repo, " (hotkey pressed)");
        },
    );

    loop {
        std::thread::sleep(StdDuration::from_secs(60));
    }
}
