//! Spirit warp-to-player (experimental; analysis and results in
//! `docs/warp_research.md`).
//!
//! The engine keeps `SummonBuddyManager.warp_manager`, a machine that
//! teleports a spirit back to the player. It only fires when the spirit's
//! line of sight to the player is blocked or the spirit is stuck - one that is
//! merely far away runs after the player on foot. Three parts here:
//!
//! - `WarpDistance` / `WarpBlockedTime` / `WarpStuckTime` / `WarpStuckRange`
//!   (0 = the game's own value, hot reload): written every second into the
//!   manager's runtime copy of the `GameSystemCommonParam` `buddyWarp_*`
//!   thresholds, so spirits warp back sooner. The game resets them on area
//!   load, hence the periodic write; its value is remembered and put back
//!   when a key goes to 0.
//! - `WarpWhenFar` (hot reload): ten times a second, a spirit of the local player
//!   that sits idle (stage 0) while one of the engine's own warp conditions
//!   holds gets its stage set to 1 (RequestWarp), see [request_warps].
//! - `WarpProbe` (diagnostics only): logs the manager's state when it changes.
//!
//! What IDA shows (2.7.1.0, `sub_1404C2890` = the warp manager's update;
//! entries = an `std::map<FieldInsHandle, entry>` at `a1 + 8` (sentinel
//! node); node: left +0, parent +8, right +16, isnil byte +25, key +32,
//! stage byte +48, flags dword +96):
//! - every frame `sub_1404C28D0` rebuilds the flag dword from zero: bit0 = a
//!   flag from the chr, bit1 = the chr has no live group entry in
//!   `SummonBuddyManager`, bit2 = the chr's `vtable[464]()->+192 ->+55752
//!   ->+40 == 82`, bit3 = path-stuck time (node +132) > `a1+32`, bit4 = a ray
//!   from the chr to the player is blocked (time accumulates in node +100),
//!   bit5 = that time > `a1+24`, bit6 = squared distance to the player >=
//!   `a1+28` squared, bit7 = some game state, bit8 = far for 1.5 s;
//! - `sub_1404C2FD0` raises the stage from 0 to 1 (RequestWarp) when
//!   (flags & 7) == 7 and one of: (bit3 and (bit4 or bit6)), bit5, or (bit7
//!   and ...) - bit 2 is often 0, so spirits blocked for 5 s still stayed;
//! - `sub_1404C3180` then moves stage 1 -> 2 (spawn point from
//!   `sub_1404C0EC0`, chr teleported) -> 3 (fade in) -> 0.
//!
//! The node layout is hard-coded, so it is checked first: [layout_known]
//! looks for the engine's own code that reads those offsets and, if it is not
//! there (the game was updated), the entry features stay off. Every node is
//! also checked to be readable before it is touched.

use std::time::Duration;

use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use fromsoftware_shared::FromStatic;

use common::{config, logger, memscan};

const TICK_INTERVAL_MS: f64 = 1000.0;
/// How often `WarpWhenFar` looks at the entries. Walking them checks every
/// node is readable, which adds up with dozens of spirits; 0.1 s is still far
/// quicker than the engine's own 1.5 s conditions.
const REQUEST_INTERVAL_MS: f64 = 100.0;
const MAX_NODES: usize = 64;
/// Bytes of a node the code reads (up to the stuck timers at +132).
const NODE_SIZE: usize = 136;

/// `mov eax,[rbx+60h]; mov ecx,eax; and ecx,7; cmp ecx,7; jnz; test al,8;
/// jz; test al,50h; jz; mov r8b,1; jmp` in `sub_1404C2FD0` (0x1404C309C,
/// unique): reads the flag dword at node +0x60.
const FLAGS_AOB: &str = "8B 43 60 8B C8 83 E1 07 83 F9 07 75 0D A8 08 74 09 A8 50 74 05 41 B0 01 EB 08";
/// `cmp byte [rbx+30h],0; jz; cmp byte [rbx+19h],0; jnz ..; mov rax,[rbx+10h];
/// cmp byte [rax+19h],0` in `sub_1404C2FD0` (0x1404C3035): reads the stage
/// byte (+0x30 from the map node's value, +48 from the node) and the isnil
/// byte (+0x19 = 25).
const STAGE_AOB: &str = "80 7B 30 00 74 ?? 80 7B 19 00 0F 85 ?? ?? ?? ?? 48 8B 43 10 80 78 19 00";

unsafe extern "system" {
    fn IsBadReadPtr(ptr: *const u8, size: usize) -> i32;
}

/// Whether `size` bytes at `addr` can be read.
fn readable(addr: usize, size: usize) -> bool {
    addr >= 0x10000 && addr % 8 == 0 && unsafe { IsBadReadPtr(addr as *const u8, size) } == 0
}

/// Whether the engine's warp code still reads the node layout this module
/// hard-codes.
fn layout_known() -> bool {
    let found = |pattern: &str| {
        memscan::wait_for_pattern_in_module(pattern, Duration::from_millis(500), Duration::from_secs(30)).is_some()
    };
    found(FLAGS_AOB) && found(STAGE_AOB)
}

unsafe fn rd<T: Copy>(base: usize, offset: usize) -> T {
    unsafe { ((base + offset) as *const T).read_unaligned() }
}

/// In-order successor of `node` in the MSVC red-black tree (nil nodes have
/// the byte at +25 set), or `None` if a link is not readable.
unsafe fn successor(mut node: usize) -> Option<usize> {
    unsafe {
        let right: usize = rd(node, 16);
        if !readable(right, 32) {
            return None;
        }
        if rd::<u8>(right, 25) == 0 {
            let mut n = right;
            for _ in 0..MAX_NODES {
                let left: usize = rd(n, 0);
                if !readable(left, 32) {
                    return None;
                }
                if rd::<u8>(left, 25) != 0 {
                    return Some(n);
                }
                n = left;
            }
            return None;
        }
        let mut parent: usize = rd(node, 8);
        for _ in 0..MAX_NODES {
            if !readable(parent, 32) {
                return None;
            }
            if rd::<u8>(parent, 25) != 0 || node != rd::<usize>(parent, 16) {
                return Some(parent);
            }
            node = parent;
            parent = rd(node, 8);
        }
        None
    }
}

/// Calls `f(node)` for each entry node of the warp manager at `a1` (at most
/// [MAX_NODES]); stops quietly at the first unreadable link.
unsafe fn for_each_node(a1: usize, mut f: impl FnMut(usize)) {
    unsafe {
        if !readable(a1, 64) {
            return;
        }
        let sentinel: usize = rd(a1, 8);
        if !readable(sentinel, 32) {
            return;
        }
        let mut node: usize = rd(sentinel, 0);
        let mut count = 0;
        while node != sentinel && count < MAX_NODES {
            if !readable(node, NODE_SIZE) {
                return;
            }
            f(node);
            count += 1;
            match successor(node) {
                Some(next) => node = next,
                None => return,
            }
        }
    }
}

/// `FieldInsHandle`s (as the raw 8 bytes the map keys use) of the spirits that
/// belong to the local player - the entries of other players' spirits (co-op)
/// must not be touched.
fn local_handles(world_chr_man: &WorldChrMan) -> Vec<u64> {
    world_chr_man
        .summon_buddy_manager
        .groups
        .iter()
        .flat_map(|pair| pair.second.iter())
        .filter(|g| !g.is_remote && !g.disappear_requested)
        .map(|g| unsafe { (&g.chr_ins.as_ref().field_ins_handle as *const _ as *const u64).read_unaligned() })
        .collect()
}

/// `WarpWhenFar`: sets the stage of an idle spirit (stage 0) of the local
/// player to 1 (RequestWarp) when one of these holds (flag bits, see the
/// module doc): far for 1.5 s (0x100, which nothing in the engine reads on its
/// own), blocked longer than `WarpBlockedTime` (0x20), or stuck (0x8) while
/// far / blocked (0x50). The engine also wants bits 0-2 all set for the last
/// two; bit 2 is often 0 (log 2026-10-09: blocked for 4.8 s, no warp), so ask
/// regardless. The engine then picks a spot near the player and moves the
/// spirit (stage 2, 3).
unsafe fn request_warps(a1: usize, world_chr_man: &WorldChrMan) {
    let mut local: Option<Vec<u64>> = None;
    unsafe {
        for_each_node(a1, |node| {
            let stage: u8 = rd(node, 48);
            let flags: u32 = rd(node, 96);
            let wanted = flags & 0x100 != 0 || flags & 0x20 != 0 || (flags & 0x8 != 0 && flags & 0x50 != 0);
            if stage != 0 || !wanted {
                return;
            }
            let key: u64 = rd(node, 32);
            let local = local.get_or_insert_with(|| local_handles(world_chr_man));
            if local.contains(&key) {
                ((node + 48) as *mut u8).write(1);
                logger::log(&format!("Warp: spirit {key:#x} (flags {flags:#05x}): warp requested."));
            }
        });
    }
}

/// One threshold of the warp manager: the ini key, how to reach the field,
/// and the bookkeeping to restore the game's value.
struct Threshold {
    key: &'static str,
    field: fn(&mut eldenring::cs::SummonBuddyWarpManager) -> &mut f32,
    /// The game's own value (last seen not written by us).
    vanilla: Option<f32>,
    /// What we last wrote.
    written: Option<f32>,
}

impl Threshold {
    fn new(key: &'static str, field: fn(&mut eldenring::cs::SummonBuddyWarpManager) -> &mut f32) -> Self {
        Threshold { key, field, vanilla: None, written: None }
    }

    /// Writes / restores this threshold in `manager`.
    fn sync(&mut self, manager: &mut eldenring::cs::SummonBuddyWarpManager) {
        let wanted = config::get_double(self.key, 0.0).max(0.0) as f32;
        let at = (self.field)(manager);
        if Some(*at) != self.written {
            // The game (re)set it: that is the vanilla value.
            self.vanilla = Some(*at);
            self.written = None;
        }
        if wanted > 0.0 {
            if *at != wanted {
                logger::log(&format!("Warp: {} {} -> {wanted}.", self.key, *at));
                *at = wanted;
            }
            self.written = Some(wanted);
        } else if let (Some(_), Some(vanilla)) = (self.written, self.vanilla) {
            *at = vanilla;
            logger::log(&format!("Warp: {} back to {vanilla}.", self.key));
            self.written = None;
        }
    }
}

/// One line describing the warp manager, or `None` if it is not readable.
fn describe(a1: usize) -> Option<String> {
    if !readable(a1, 64) {
        return None;
    }
    unsafe {
        let mut text = format!(
            "WarpProbe: manager ray_time={} dist={} stack_time={} force={}; entries:",
            rd::<f32>(a1, 24),
            rd::<f32>(a1, 28),
            rd::<f32>(a1, 32),
            rd::<u8>(a1, 48)
        );
        let mut count = 0;
        for_each_node(a1, |node| {
            text.push_str(&format!(
                " [key {:#x} stage {} flags {:#05x} ray {:.1} timer {:.1} stack {:.1}/{:.1}]",
                rd::<u64>(node, 32),
                rd::<u8>(node, 48),
                rd::<u32>(node, 96),
                rd::<f32>(node, 100),
                rd::<f32>(node, 104),
                rd::<f32>(node, 128),
                rd::<f32>(node, 132)
            ));
            count += 1;
        });
        if count == 0 {
            text.push_str(" none");
        }
        Some(text)
    }
}

pub fn run() {
    if !layout_known() {
        logger::error("Warp: the engine's warp code was not recognised (game updated?) - warp features off.");
        return;
    }
    let cs_task = common::task::wait_for_cs_task();
    let mut elapsed_ms: f64 = TICK_INTERVAL_MS;
    let mut request_ms: f64 = 0.0;
    let mut last = String::new();
    let mut when_far = false;
    let mut thresholds = [
        Threshold::new("WarpDistance", |m| &mut m.trigger_dist_to_player),
        Threshold::new("WarpBlockedTime", |m| &mut m.trigger_time_ray_block),
        Threshold::new("WarpStuckTime", |m| &mut m.trigger_threshold_time_path_stacked),
        Threshold::new("WarpStuckRange", |m| &mut m.trigger_threshold_range_path_stacked),
    ];

    let _handle = common::task::run_recurring_safe(
        cs_task,
        "Warp",
        CSTaskGroupIndex::FrameBegin,
        move |data: &eldenring::fd4::FD4TaskData| {
            if common::player::main_player_chr_ins_ptr().is_none() {
                return;
            }
            let Ok(world_chr_man) = (unsafe { WorldChrMan::instance_mut() }) else {
                return;
            };
            let a1 = &*world_chr_man.summon_buddy_manager.warp_manager as *const _ as usize;
            if a1 == 0 {
                return;
            }

            let frame_ms = (data.delta_time.time as f64) * 1000.0;

            // Once a second: ini values, thresholds, diagnostics.
            elapsed_ms += frame_ms;
            if elapsed_ms >= TICK_INTERVAL_MS {
                elapsed_ms = 0.0;
                when_far = config::get_bool("WarpWhenFar", false);
                for threshold in thresholds.iter_mut() {
                    threshold.sync(&mut world_chr_man.summon_buddy_manager.warp_manager);
                }
                if config::get_bool("WarpProbe", false) {
                    if let Some(text) = describe(a1) {
                        if text != last {
                            logger::log(&text);
                            last = text;
                        }
                    }
                }
            }

            // Ten times a second: a warp request should not wait a whole second.
            request_ms += frame_ms;
            if request_ms >= REQUEST_INTERVAL_MS {
                request_ms = 0.0;
                if when_far {
                    unsafe { request_warps(a1, world_chr_man) };
                }
            }
        },
    );
    logger::log("Warp: tick registered on CSTaskGroupIndex::FrameBegin.");

    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
