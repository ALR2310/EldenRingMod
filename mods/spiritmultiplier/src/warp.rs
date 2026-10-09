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

use common::{codepatch, config, logger, memscan};

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

/// `cmp qword [r15+0E8h],0; jz ..; call sub_1404B7710; test al,al; jnz ..;
/// mov rdx,[rdi+8]; mov rcx,[r15+0E8h]; call sub_1404C0DD0` at the end of
/// `sub_1404BAEA0` (0x1404BB91B, unique): right after a spirit's chr is
/// created, the game registers it with the warp manager. The `call` to the
/// register function is at +30.
const REGISTER_AOB: &str =
    "49 83 BF E8 00 00 00 00 74 ?? E8 ?? ?? ?? ?? 84 C0 75 ?? 48 8B 57 08 49 8B 8F E8 00 00 00 E8 ?? ?? ?? ??";
const REGISTER_CALL_OFFSET: usize = 30;

/// `cmp qword [rcx+0E8h],0; mov rdi,rdx; mov rsi,rcx; jz ..; call
/// sub_1404B7710; test al,al; jnz ..; mov rdx,[rdi+8]; mov rcx,[rsi+0E8h];
/// call sub_1404C26F0` in `sub_1404BBAA0` (0x1404BBAAA, unique): when a
/// spirit leaves its group the game removes its warp entry. The `call` to the
/// unregister function is at +36.
const UNREGISTER_AOB: &str =
    "48 83 B9 E8 00 00 00 00 48 8B FA 48 8B F1 74 ?? E8 ?? ?? ?? ?? 84 C0 75 ?? 48 8B 57 08 48 8B 8E E8 00 00 00 E8 ?? ?? ?? ??";
const UNREGISTER_CALL_OFFSET: usize = 36;

/// `lea rdx,[rsp+70h]; mov rcx,[r15+0E8h]; call sub_1404C2890; nop; mov
/// [rsp+70h],rbx; mov [rsp+70h],rdi` in `SummonBuddyManager::Update`
/// (0x1404B8B6D, unique): the per-frame warp manager update (flags, stage
/// 0 -> 1, the stage 1 -> 2 -> 3 move). The `call` is at +12.
const UPDATE_AOB: &str = "48 8D 54 24 70 49 8B 8F E8 00 00 00 E8 ?? ?? ?? ?? 90 48 89 5C 24 70 48 89 7C 24 70";
const UPDATE_CALL_OFFSET: usize = 12;

/// The time argument of the update: the engine's `FD4Time` (vftable, then
/// the frame time in seconds at +8); the warp code only reads the float.
#[repr(C)]
struct FrameTime {
    vftable: usize,
    seconds: f32,
    pad: u32,
}

type UpdateFn = unsafe extern "system" fn(manager: usize, time: *const FrameTime) -> i64;

/// `sub_1404C0DD0(warp_manager, field_ins_handle)`: adds an entry for the
/// handle (stage 0) unless there is one. `sub_1404C26F0` (same signature)
/// removes it.
type RegisterFn = unsafe extern "system" fn(manager: usize, handle: u64) -> i64;

/// Both game functions, resolved from the call sites above.
#[derive(Clone, Copy)]
struct Registry {
    register: RegisterFn,
    unregister: RegisterFn,
    update: UpdateFn,
}

/// Whether Seamless Co-op (`ersc.dll`) is loaded.
fn seamless_loaded() -> bool {
    common::diag::loaded_modules().iter().any(|m| m.name.eq_ignore_ascii_case("ersc.dll"))
}

/// The target of the `call` at `offset` in the first match of `pattern`.
fn call_target(pattern: &str, offset: usize) -> Option<usize> {
    let at = memscan::wait_for_pattern_in_module(pattern, Duration::from_millis(500), Duration::from_secs(30))?;
    codepatch::rel32_target(unsafe { at.add(offset) }).map(|target| target as usize)
}

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
        .filter(|g| !g.is_remote && crate::multi_spirit::group_alive(g))
        .map(|g| unsafe { (&g.chr_ins.as_ref().field_ins_handle as *const _ as *const u64).read_unaligned() })
        .collect()
}

/// Registers with the warp manager every spirit of the local player that has
/// no entry. The game does it when it creates the spirit's chr, but with
/// Seamless Co-op the spirits are created by Seamless's own code, which skips
/// that call: no entry, so the engine never warps them (log 2026-10-09, the
/// same ini with and without Seamless: `entries: none` with 3 spirits out).
/// Does nothing when every spirit already has its entry (vanilla).
///
/// `ours` = the handles registered here: when such a spirit is gone (dead,
/// sent back, despawned) its entry is removed again, as the game would have
/// done - the update looks the chr up by handle and must not find a stale
/// entry.
unsafe fn register_missing(a1: usize, world_chr_man: &WorldChrMan, registry: Registry, ours: &mut Vec<u64>) {
    let mut have: Vec<u64> = Vec::new();
    unsafe { for_each_node(a1, |node| have.push(rd(node, 32))) };
    let local = local_handles(world_chr_man);
    for &handle in &local {
        if !have.contains(&handle) {
            unsafe { (registry.register)(a1, handle) };
            ours.push(handle);
            logger::log(&format!("Warp: registered spirit {handle:#x} with the warp manager."));
        }
    }
    ours.retain(|handle| {
        if local.contains(handle) {
            return true;
        }
        if have.contains(handle) {
            unsafe { (registry.unregister)(a1, *handle) };
            logger::log(&format!("Warp: removed spirit {handle:#x} from the warp manager."));
        }
        false
    });
}

/// Distance to the player of each live spirit of the local player, by handle.
fn local_distances(world_chr_man: &WorldChrMan) -> Vec<(u64, f32)> {
    let Some(player) = world_chr_man.main_player.as_ref() else {
        return Vec::new();
    };
    let at = player.chr_ins.modules.physics.position;
    world_chr_man
        .summon_buddy_manager
        .groups
        .iter()
        .flat_map(|pair| pair.second.iter())
        .filter(|g| !g.is_remote && crate::multi_spirit::group_alive(g))
        .map(|g| {
            let chr = unsafe { g.chr_ins.as_ref() };
            let handle = unsafe { (&chr.field_ins_handle as *const _ as *const u64).read_unaligned() };
            let p = chr.modules.physics.position;
            let (dx, dy, dz) = (p.0 - at.0, p.1 - at.1, p.2 - at.2);
            (handle, (dx * dx + dy * dy + dz * dz).sqrt())
        })
        .collect()
}

/// How long a spirit must stay farther than the warp distance (the engine's
/// own "far" flag uses the same 1.5 s).
const FAR_SECONDS: f32 = 1.5;

/// For the spirits registered by this module (`ours`): the engine's own
/// conditions are not met for them (log 2026-10-09 with Seamless: every flag
/// stayed 0, even far away), so the distance is measured here and a spirit
/// that stays farther than the manager's warp distance for [FAR_SECONDS] gets
/// its stage set to 1 (RequestWarp). `far_for` = seconds each has been far.
unsafe fn request_far_ours(
    a1: usize,
    world_chr_man: &WorldChrMan,
    ours: &[u64],
    far_for: &mut std::collections::HashMap<u64, f32>,
    dt: f32,
) {
    let limit = world_chr_man.summon_buddy_manager.warp_manager.trigger_dist_to_player;
    let mut due: Vec<(u64, f32)> = Vec::new();
    far_for.retain(|handle, _| ours.contains(handle));
    for (handle, dist) in local_distances(world_chr_man) {
        if !ours.contains(&handle) {
            continue;
        }
        if dist >= limit {
            let time = far_for.entry(handle).or_insert(0.0);
            *time += dt;
            if *time >= FAR_SECONDS {
                due.push((handle, dist));
            }
        } else {
            far_for.remove(&handle);
        }
    }
    if due.is_empty() {
        return;
    }
    unsafe {
        for_each_node(a1, |node| {
            let key: u64 = rd(node, 32);
            if let Some(&(_, dist)) = due.iter().find(|(handle, _)| *handle == key) {
                if rd::<u8>(node, 48) == 0 {
                    ((node + 48) as *mut u8).write(1);
                    far_for.remove(&key);
                    logger::log(&format!("Warp: spirit {key:#x} is {dist:.0} m away: warp requested."));
                }
            }
        });
    }
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
    // Only with Seamless Co-op: the game registers vanilla spirits itself (and
    // not on purpose in the arena), so there is nothing to fix without it.
    // Checked after `CSTaskImp` exists, when every mod loader has loaded its DLLs.
    let registry = if seamless_loaded() {
        match (
            call_target(REGISTER_AOB, REGISTER_CALL_OFFSET),
            call_target(UNREGISTER_AOB, UNREGISTER_CALL_OFFSET),
            call_target(UPDATE_AOB, UPDATE_CALL_OFFSET),
        ) {
            (Some(register), Some(unregister), Some(update)) => {
                logger::log("Warp: Seamless Co-op found - its spirits are registered for warping here.");
                Some(unsafe {
                    Registry {
                        register: std::mem::transmute::<usize, RegisterFn>(register),
                        unregister: std::mem::transmute::<usize, RegisterFn>(unregister),
                        update: std::mem::transmute::<usize, UpdateFn>(update),
                    }
                })
            }
            _ => {
                logger::error("Warp: the game's spirit registration was not found - Seamless spirits will not warp.");
                None
            }
        }
    } else {
        None
    };
    let mut ours: Vec<u64> = Vec::new();
    let mut far_for: std::collections::HashMap<u64, f32> = std::collections::HashMap::new();
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

            // The engine does not run the warp update for the spirits this
            // module registered (Seamless: every flag stayed 0 and a requested
            // warp stayed at stage 1, log 2026-10-09), so run it here, once a
            // frame, as `SummonBuddyManager::Update` would.
            if let (Some(registry), false) = (registry, ours.is_empty()) {
                let time = FrameTime { vftable: 0, seconds: data.delta_time.time, pad: 0 };
                unsafe { (registry.update)(a1, &time) };
            }

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
                let dt = (request_ms / 1000.0) as f32;
                request_ms = 0.0;
                // Often, so a spirit that just left is dropped before the engine's
                // update looks for its chr again.
                if let Some(registry) = registry {
                    unsafe { register_missing(a1, world_chr_man, registry, &mut ours) };
                    unsafe { request_far_ours(a1, world_chr_man, &ours, &mut far_for, dt) };
                }
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
