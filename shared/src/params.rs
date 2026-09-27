//! Row access for `SoloParamRepository` params that doesn't go through
//! fromsoftware-rs's `rows()`/`rows_mut()`.
//!
//! Those iterators walk the runtime lookup table the game appends after each
//! param file (sorted by ID, `(param_id, index)` pairs), sized by the file
//! header's own `row_count`, and `.unwrap()` every index it yields. But the
//! game builds that table itself and sizes it by its own runtime count (the
//! `u32` in the metadata before the file), which can be smaller: with
//! Convergence's regulation.bin, `ItemLotParam_enemy`'s header says 4631 rows
//! while the runtime count is 4630 (most likely a duplicate row ID the game
//! drops when building the table). fromsoftware-rs then reads one entry past
//! the real table, gets a garbage index past `row_count`, and panics inside
//! `rows_mut()` (`param_repository.rs:357`), killing the calling thread.
//! Reported on Nexus for DropMultiplier 1.0.1 (2026-09-26, 1.17 +
//! Convergence), reproduced and confirmed here on 1.16.2 + Convergence
//! (2026-09-27); vanilla 1.17 (5135 rows) is fine.
//!
//! [`for_each_row_mut`] walks the file's own row descriptors by index
//! instead (the same data `get_row_by_index_mut` already reads), which never
//! touches the lookup table and simply stops at `row_count`. The trade-off
//! is that callers get a row index, not a param ID - fine for anything that
//! only needs a stable key within one session (e.g. a hot-reload baseline
//! snapshot), not for looking up a specific row by ID.

use eldenring::cs::{SoloParam, SoloParamRepository};
use eldenring::param::ParamDef;

/// One-line description of `P`'s slot in `repo` for the log: resource name,
/// struct name, paramdef version and row count - so a bug-report log shows
/// what the game actually has at that slot.
pub fn describe<P: SoloParam>(repo: &SoloParamRepository) -> String {
    let Some(holder) = repo.solo_param_holders.get(P::INDEX as usize) else {
        return format!("{}: holder index {} out of range", P::NAME, P::INDEX);
    };
    let Some(res_cap) = holder.get_res_cap(0) else {
        return format!("{}: holder {} has no res cap", P::NAME, P::INDEX);
    };
    let file = &res_cap.param_res_cap.data;
    format!(
        "{} (slot {}): resource '{}', struct '{}' v{}, {} row(s) (runtime: {})",
        P::NAME,
        P::INDEX,
        res_cap.res_cap.name,
        file.struct_name(),
        file.paramdef_version(),
        file.row_count(),
        runtime_row_count(file)
    )
}

/// The `u32` row count the game keeps in the runtime metadata 0x10 bytes
/// before the param file (fromsoftware-rs's private `ParamFileMetadata`,
/// `after_name_offset: u32` then `row_count: u32`). The file header's own
/// `row_count` is only a `u16`; this is what the lookup table is sized by.
fn runtime_row_count(file: &eldenring::fd4::ParamFile) -> u32 {
    unsafe {
        let metadata = (file as *const _ as *const u8).sub(0x10);
        (metadata.add(4) as *const u32).read_unaligned()
    }
}

/// Checks that the slot fromsoftware-rs hardcodes for `P` really holds `P`
/// (resource name and struct name both match). fromsoftware-rs only does
/// this with `debug_assert!`, i.e. never in a release build - on a game
/// version whose param order differs from the one fromsoftware-rs was
/// written against, writing through the wrong slot would silently corrupt
/// some other param instead.
pub fn check<P: SoloParam>(repo: &SoloParamRepository) -> Result<(), String> {
    let res_cap = repo
        .solo_param_holders
        .get(P::INDEX as usize)
        .and_then(|holder| holder.get_res_cap(0))
        .ok_or_else(|| describe::<P>(repo))?;
    let struct_name = res_cap.param_res_cap.data.struct_name();
    if res_cap.res_cap.name != P::NAME || struct_name != P::StructType::NAME {
        return Err(format!(
            "unexpected param at slot {}: {} (expected '{}' / '{}')",
            P::INDEX,
            describe::<P>(repo),
            P::NAME,
            P::StructType::NAME
        ));
    }
    Ok(())
}

/// Calls `f(index, row)` for every row of `P`, in file order, by row index
/// (see the module doc for why not `rows_mut()`). Returns the row count.
pub fn for_each_row_mut<P: SoloParam>(
    repo: &mut SoloParamRepository,
    mut f: impl FnMut(usize, &mut P::StructType),
) -> usize {
    let mut index = 0;
    while let Some(row) = repo.get_row_by_index_mut::<P>(index) {
        f(index, row);
        index += 1;
    }
    index
}
