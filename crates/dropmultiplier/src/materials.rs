//! Multiplies how many crafting/upgrade materials an item lot hands out
//! (`lot_item_num0N`), in both `ItemLotParam_map` (gathering spots, items
//! placed in the world) and `ItemLotParam_enemy` (enemy/animal drops) - the
//! `[Materials]` ini section, independent of `drop_rate`'s drop *chance*.
//! Suggested on Nexus (2026-09-27) after the Convergence fix.
//!
//! Each lot slot falls into one of three groups, or none:
//! - not a Goods slot (`lot_item_category0N != 1`), or a Goods item whose
//!   `EquipParamGoods.goodsType` is neither 2 (crafting material) nor 14
//!   (upgrade material: Smithing Stones, Gloveworts, Golden Seeds, Sacred
//!   Tears, Scadutree Fragments, ...) -> untouched. This is what keeps
//!   weapons, key items, consumables and odd rows like `ItemLotParam_map`
//!   row 2 (a random Crimson Tears flask lottery with no pickup flag) out;
//! - the row or slot has a pickup event flag (`getItemFlagId` /
//!   `getItemFlagId0N` != 0), i.e. can only be obtained once -> `Unique`;
//! - otherwise it respawns after resting at a grace -> `Crafting` or
//!   `Upgrade` by goods type.
//!
//! Derived from a vanilla 1.17 export (2026-09-28): gathering spots are the
//! unflagged `ItemLotParam_map` rows 9965xx-9993xx (base game) and 463xxxx
//! (DLC), e.g. 997200 Rowa Fruit rolls 1/2/3/5 - scaling `lot_item_num`
//! scales every one of those outcomes. Farmable Smithing Stones come from
//! respawning enemies (miners, golems, soldiers) in `ItemLotParam_enemy`;
//! Smithing Stones on corpses, Sacred Tears and Scadutree Fragments are all
//! flagged, so land in `Unique`.
//!
//! Goods types are read from the live `EquipParamGoods` at runtime (never a
//! hardcoded ID list), so overhaul regulations like Convergence classify
//! their own items correctly.
//!
//! Same hot-reload baseline pattern as `drop_rate`: original counts and each
//! slot's group are snapshotted once, every apply recomputes from that.

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{EquipParamGoods, ItemLotParam_enemy, ItemLotParam_map, SoloParam, SoloParamRepository};
use eldenring::param::ITEMLOT_PARAM_ST;

use common::config;
use common::logger;

const CATEGORY_GOODS: i32 = 1;
const GOODS_TYPE_CRAFTING: u8 = 2;
const GOODS_TYPE_UPGRADE: u8 = 14;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    None,
    Crafting,
    Upgrade,
    Unique,
}

struct RowSnapshot {
    nums: [u8; 8],
    groups: [Group; 8],
}

struct Snapshot {
    enemy: Vec<RowSnapshot>,
    map: Vec<RowSnapshot>,
}

static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);

struct Factors {
    crafting: f64,
    upgrade: f64,
    unique: f64,
}

impl Factors {
    fn from_config() -> Self {
        Self {
            crafting: config::get_double("Crafting", 1.0).max(0.0),
            upgrade: config::get_double("Upgrade", 1.0).max(0.0),
            unique: config::get_double("Unique", 1.0).max(0.0),
        }
    }

    fn get(&self, group: Group) -> Option<f64> {
        match group {
            Group::None => None,
            Group::Crafting => Some(self.crafting),
            Group::Upgrade => Some(self.upgrade),
            Group::Unique => Some(self.unique),
        }
    }
}

fn get_nums(row: &ITEMLOT_PARAM_ST) -> [u8; 8] {
    [
        row.lot_item_num01(),
        row.lot_item_num02(),
        row.lot_item_num03(),
        row.lot_item_num04(),
        row.lot_item_num05(),
        row.lot_item_num06(),
        row.lot_item_num07(),
        row.lot_item_num08(),
    ]
}

fn set_nums(row: &mut ITEMLOT_PARAM_ST, nums: [u8; 8]) {
    row.set_lot_item_num01(nums[0]);
    row.set_lot_item_num02(nums[1]);
    row.set_lot_item_num03(nums[2]);
    row.set_lot_item_num04(nums[3]);
    row.set_lot_item_num05(nums[4]);
    row.set_lot_item_num06(nums[5]);
    row.set_lot_item_num07(nums[6]);
    row.set_lot_item_num08(nums[7]);
}

fn get_categories(row: &ITEMLOT_PARAM_ST) -> [i32; 8] {
    [
        row.lot_item_category01(),
        row.lot_item_category02(),
        row.lot_item_category03(),
        row.lot_item_category04(),
        row.lot_item_category05(),
        row.lot_item_category06(),
        row.lot_item_category07(),
        row.lot_item_category08(),
    ]
}

fn get_slot_flags(row: &ITEMLOT_PARAM_ST) -> [u32; 8] {
    [
        row.get_item_flag_id01(),
        row.get_item_flag_id02(),
        row.get_item_flag_id03(),
        row.get_item_flag_id04(),
        row.get_item_flag_id05(),
        row.get_item_flag_id06(),
        row.get_item_flag_id07(),
        row.get_item_flag_id08(),
    ]
}

/// `EquipParamGoods` row ID -> `goodsType`, walked by index through
/// [`common::params::row_ids`] rather than `repo.get()` (see that module).
fn goods_types(repo: &SoloParamRepository) -> Option<HashMap<u32, u8>> {
    let ids = common::params::row_ids::<EquipParamGoods>(repo)?;
    let mut types = HashMap::with_capacity(ids.len());
    for (index, id) in ids.into_iter().enumerate() {
        types.insert(id, repo.get_row_by_index::<EquipParamGoods>(index)?.goods_type());
    }
    Some(types)
}

fn classify(row: &ITEMLOT_PARAM_ST, goods_types: &HashMap<u32, u8>) -> RowSnapshot {
    let item_ids = crate::drop_rate::get_item_ids(row);
    let categories = get_categories(row);
    let slot_flags = get_slot_flags(row);
    let row_flag = row.get_item_flag_id();
    let mut groups = [Group::None; 8];
    for i in 0..8 {
        if item_ids[i] <= 0 || categories[i] != CATEGORY_GOODS {
            continue;
        }
        let farmable_group = match goods_types.get(&(item_ids[i] as u32)) {
            Some(&GOODS_TYPE_CRAFTING) => Group::Crafting,
            Some(&GOODS_TYPE_UPGRADE) => Group::Upgrade,
            _ => continue,
        };
        groups[i] = if row_flag != 0 || slot_flags[i] != 0 { Group::Unique } else { farmable_group };
    }
    RowSnapshot { nums: get_nums(row), groups }
}

fn snapshot_table<P: SoloParam<StructType = ITEMLOT_PARAM_ST>>(
    repo: &mut SoloParamRepository,
    goods_types: &HashMap<u32, u8>,
) -> Vec<RowSnapshot> {
    let mut rows = Vec::new();
    common::params::for_each_row_mut::<P>(repo, |_, row| rows.push(classify(row, goods_types)));
    rows
}

/// Rescales one table from its snapshot. Returns how many slots it touched.
fn apply_table<P: SoloParam<StructType = ITEMLOT_PARAM_ST>>(
    repo: &mut SoloParamRepository,
    snapshot: &[RowSnapshot],
    factors: &Factors,
) -> usize {
    let mut changed = 0;
    common::params::for_each_row_mut::<P>(repo, |index, row| {
        let Some(original) = snapshot.get(index) else {
            return;
        };
        let mut nums = original.nums;
        for i in 0..8 {
            let Some(factor) = factors.get(original.groups[i]) else {
                continue;
            };
            if original.nums[i] == 0 {
                continue; // no count to scale
            }
            // At least 1 (a factor below 1 must not turn a pickup into
            // nothing), at most 255 (`lot_item_num` is a u8).
            nums[i] = (original.nums[i] as f64 * factor).round().clamp(1.0, u8::MAX as f64) as u8;
            changed += 1;
        }
        set_nums(row, nums);
    });
    changed
}

/// Applies `[Materials]` to both item lot tables. Logs its own result; does
/// nothing (logged as an error) if a param slot isn't what we expect or
/// `EquipParamGoods`' IDs can't be read.
pub fn apply(repo: &mut SoloParamRepository, suffix: &str) {
    for check in [
        common::params::check::<ItemLotParam_enemy>(repo),
        common::params::check::<ItemLotParam_map>(repo),
        common::params::check::<EquipParamGoods>(repo),
    ] {
        if let Err(reason) = check {
            logger::error(&format!("{reason} - material amounts left unchanged."));
            return;
        }
    }

    let mut snapshot_guard = SNAPSHOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if snapshot_guard.is_none() {
        let Some(goods_types) = goods_types(repo) else {
            logger::error(&format!(
                "could not read item IDs of {} - material amounts left unchanged.",
                common::params::describe::<EquipParamGoods>(repo)
            ));
            return;
        };
        let snapshot = Snapshot {
            enemy: snapshot_table::<ItemLotParam_enemy>(repo, &goods_types),
            map: snapshot_table::<ItemLotParam_map>(repo, &goods_types),
        };
        let count = |group: Group| {
            snapshot.enemy.iter().chain(&snapshot.map).flat_map(|r| r.groups).filter(|&g| g == group).count()
        };
        logger::log(&format!(
            "Materials: {} ({} goods), found {} crafting, {} upgrade, {} unique material slot(s).",
            common::params::describe::<ItemLotParam_map>(repo),
            goods_types.len(),
            count(Group::Crafting),
            count(Group::Upgrade),
            count(Group::Unique)
        ));
        *snapshot_guard = Some(snapshot);
    }
    let Some(snapshot) = snapshot_guard.as_ref() else {
        return;
    };

    let factors = Factors::from_config();
    let changed = apply_table::<ItemLotParam_enemy>(repo, &snapshot.enemy, &factors)
        + apply_table::<ItemLotParam_map>(repo, &snapshot.map, &factors);
    logger::log(&format!(
        "Crafting={:.3} Upgrade={:.3} Unique={:.3} applied to {changed} material slot(s){suffix}.",
        factors.crafting, factors.upgrade, factors.unique
    ));
}
