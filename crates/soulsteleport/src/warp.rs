//! Offline proof-of-concept for "warp to an exact position, with a loading
//! screen" - the mechanism a future SoulsChat `/teleport <player>` addon
//! needs (see README). `SaveKey` remembers the main player's current block +
//! block-local position; `WarpKey` asks the game to move-map back there.
//!
//! Reuses the game's own sequence, found by static analysis of eldenring.exe
//! 2.7.1.0 (the game calls it when a multiplayer session ends, to put you back
//! where you stood - sub_1405F36E0/sub_1405F34A0):
//! 1. `RequestMoveMap` (sub_14067BA20) - writes `GameMan.move_map_target`,
//!    normalizing overworld (area 50..88) block ids the way the engine expects.
//! 2. `SetCustomSpawn` (sub_14067B970) - writes `GameMan+0xC90` (block-local
//!    position, w = 1.0), `+0xCA0` (orientation) and sets the `+0xCB0` flag the
//!    post-load spawn resolver (sub_140AFE280) checks before falling back to
//!    region/grace spawn points.
//! 3. `GameMan.warp_requested = true` - done directly rather than through the
//!    game's own trigger (sub_1405F89C0), which also calls into the session
//!    manager when online and might drop a Seamless Co-op session.
//!
//! No version lock: both functions are found by AOB (same unique pattern in
//! exe 2.6.2.0, 2.7.0.0 and 2.7.1.0), and the `GameMan` static itself is read
//! off `SetCustomSpawn`'s own `mov rax, [rip+disp32]` - fromsoftware-rs's
//! `GameMan::instance()` goes through its version-gated RVA table and panics
//! on any exe it has no table for (it did, on 2.7.0.0).

use std::sync::Mutex;

use eldenring::cs::{CSTaskGroupIndex, WorldChrMan};
use eldenring::fd4::FD4TaskData;
// Not `common::input::is_key_pressed`: that one polls from a plain thread
// via `GetAsyncKeyState`, which (before 2026-09-24) let 2 game instances on
// one PC steal each other's presses. This runs on the game's own task, where
// fromsoftware-rs's `GetKeyState`-based poll only sees keys sent to this
// process's own window - same as `common::reload` uses.
use eldenring::util::input;
use fromsoftware_shared::FromStatic;

use common::input::parse_virtual_key;
use common::{announce, config, logger, memscan};

use crate::steam::SteamMessages;
use crate::sync;

const VK_F7: i32 = 0x76;
const VK_F8: i32 = 0x77;
const VK_F9: i32 = 0x78;
const VK_F10: i32 = 0x79;

// ~10s at 60fps - Steam is up long before the player can reach the world.
const STEAM_LOAD_ATTEMPTS: u32 = 600;

// sub_14067BA20 in 2.7.1.0. The `cmp byte [rcx+0xB28], 0` tail pins the
// GameMan field layout the function (and the offsets below) assume.
const AOB_REQUEST_MOVE_MAP: &str =
    "40 53 48 83 EC 20 8B 02 48 8B D9 89 01 48 8B 0D ?? ?? ?? ?? 80 B9 28 0B 00 00 00";
// sub_14067B970 in 2.7.1.0 - writes +0xC90 / +0xCA0 and sets the +0xCB0 flag.
const AOB_SET_CUSTOM_SPAWN: &str = "0F 28 01 48 8B 05 ?? ?? ?? ?? 0F 11 80 90 0C 00 00 \
     0F 28 02 0F 11 80 A0 0C 00 00 C6 80 B0 0C 00 00 01";
// Offset of `mov rax, [rip+disp32]`'s disp32 within AOB_SET_CUSTOM_SPAWN, and
// of the instruction right after it (what disp32 is relative to).
const GAME_MAN_DISP_OFFSET: usize = 6;
const GAME_MAN_NEXT_INSN_OFFSET: usize = 10;

/// `GameMan.warp_requested` (fromsoftware-rs `cs/game_man.rs`; the game's
/// own setter sub_14067BCF0 is `mov [rax+10h], cl`).
const GAME_MAN_WARP_REQUESTED: usize = 0x10;

// sub_14067BA20(out, &block, unused) -> out
type RequestMoveMapFn = unsafe extern "C" fn(out: *mut i32, block: *const i32, unused: u64) -> *mut i32;
// sub_14067B970(&position, &orientation) - both read with `movaps`, so the
// pointers must be 16-byte aligned (see [Vec4]).
type SetCustomSpawnFn = unsafe extern "C" fn(position: *const Vec4, orientation: *const Vec4) -> u64;

#[repr(C, align(16))]
struct Vec4([f32; 4]);

/// A map block + block-local position (what `PlayerIns.current_block_id` /
/// `block_position` hold for the main player), i.e. exactly what the game's
/// own custom-spawn warp takes.
#[derive(Clone, Copy)]
pub struct Spot {
    pub block_id: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
}

static SAVED: Mutex<Option<Spot>> = Mutex::new(None);

pub struct GameFns {
    request_move_map: RequestMoveMapFn,
    set_custom_spawn: SetCustomSpawnFn,
    /// Address of the static holding the `GameMan*` (not GameMan itself -
    /// that's only allocated once the game boots far enough).
    game_man_static: *const usize,
}

// Raw pointers aren't Send by default; these are process-lifetime addresses
// inside eldenring.exe's image, read only from the game's own task thread.
unsafe impl Send for GameFns {}

fn resolve_game_fns() -> Option<GameFns> {
    let request_move_map = memscan::find_pattern_in_module(AOB_REQUEST_MOVE_MAP)?;
    let set_custom_spawn = memscan::find_pattern_in_module(AOB_SET_CUSTOM_SPAWN)?;
    let game_man_static = unsafe {
        let disp = (set_custom_spawn.add(GAME_MAN_DISP_OFFSET) as *const i32).read_unaligned();
        set_custom_spawn.add(GAME_MAN_NEXT_INSN_OFFSET).offset(disp as isize) as *const usize
    };
    Some(GameFns {
        request_move_map: unsafe { std::mem::transmute::<*mut u8, RequestMoveMapFn>(request_move_map) },
        set_custom_spawn: unsafe { std::mem::transmute::<*mut u8, SetCustomSpawnFn>(set_custom_spawn) },
        game_man_static,
    })
}

fn current_spot() -> Option<Spot> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    let player = world_chr_man.main_player.as_ref()?;
    let pos = &player.block_position;
    Some(Spot {
        block_id: player.current_block_id.0,
        x: pos.x,
        y: pos.y,
        z: pos.z,
        yaw: pos.yaw,
    })
}

fn save_current_spot() {
    let Some(spot) = current_spot() else {
        return;
    };
    *SAVED.lock().unwrap() = Some(spot);
    logger::log(&format!(
        "Saved spot: block {:#010X} local ({:.2}, {:.2}, {:.2}) yaw {:.3}",
        spot.block_id, spot.x, spot.y, spot.z, spot.yaw
    ));
    announce::show_announcement("Teleport spot saved");
}

/// Asks the game to move-map to `spot` (loading screen, then spawn exactly
/// there). Returns false if not in the world yet / GameMan not allocated.
pub fn warp_to(fns: &GameFns, spot: &Spot) -> bool {
    // Same "actually in the world" gate as every other mod here - warping
    // from a loading screen or the title menu is not something to test.
    if common::player::main_player_chr_ins_ptr().is_none() {
        return false;
    }
    let game_man = unsafe { *fns.game_man_static };
    if game_man == 0 {
        logger::error("Warp: GameMan not allocated yet.");
        return false;
    }

    let mut normalized_block = 0i32;
    let position = Vec4([spot.x, spot.y, spot.z, 1.0]);
    // Same layout the game builds for multiplay_join_orientation in
    // sub_1406FC370: (0, yaw, 0, 0).
    let orientation = Vec4([0.0, spot.yaw, 0.0, 0.0]);
    unsafe {
        (fns.request_move_map)(&mut normalized_block, &spot.block_id, 0);
        (fns.set_custom_spawn)(&position, &orientation);
        *((game_man + GAME_MAN_WARP_REQUESTED) as *mut bool) = true;
    }

    logger::log(&format!(
        "Warp requested: block {:#010X} (normalized {:#010X}) local ({:.2}, {:.2}, {:.2})",
        spot.block_id, normalized_block, spot.x, spot.y, spot.z
    ));
    true
}

fn warp_to_saved(fns: &GameFns) {
    let Some(spot) = *SAVED.lock().unwrap() else {
        announce::show_announcement("No teleport spot saved yet");
        return;
    };
    warp_to(fns, &spot);
}

/// Test stand-in for the future `/teleport <player>`: warps to the partner
/// whose synced position arrived most recently.
fn warp_to_partner(fns: &GameFns) {
    let Some((steam_id, remote)) = sync::fresh_remote_spots()
        .into_iter()
        .max_by_key(|(_, r)| r.received)
    else {
        announce::show_announcement("No partner position received yet");
        logger::log("Warp to partner: no fresh position from any partner.");
        return;
    };
    // No offset needed: the game itself pushes apart 2 characters spawned
    // on top of each other (confirmed by the user in-game).
    let spot = remote.spot;
    logger::log(&format!(
        "Warp to partner '{}' ({steam_id}), position {:.1}s old.",
        remote.character_name,
        remote.received.elapsed().as_secs_f32()
    ));
    if warp_to(fns, &spot) {
        announce::show_announcement(&format!("Teleporting to {}", remote.character_name));
    }
}

/// Logs every entry of `WorldChrMan.player_chr_set`, `CSSessionManager`'s
/// session members, and every synced partner position. Research/debug aid.
fn list_players() {
    let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
        return;
    };
    let mut count = 0;
    for player in world_chr_man.player_chr_set.characters() {
        count += 1;
        let name = unsafe { player.player_game_data.as_ref() }.character_name;
        let name_len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
        let name = String::from_utf16_lossy(&name[..name_len]);
        let block_pos = &player.block_position;
        let havok = &player.chr_ins.modules.physics.position;
        // Co-op test (2026-09-24): `PlayerIns.current_block_id` /
        // `block_position` are only maintained for the main player (-1 / 0
        // for a Seamless Co-op partner), and a far partner's havok position
        // drops to 0 - hence the network sync in `sync`.
        let chr = &player.chr_ins;
        let chunk = &chr.chunk_position;
        logger::log(&format!(
            "Player #{count}: '{name}' type {:?} block {:#010X} local ({:.2}, {:.2}, {:.2}) \
             havok ({:.2}, {:.2}, {:.2}) | chr block {:#010X} origin {:#010X} chunk ({:.2}, {:.2}, {:.2})",
            chr.chr_type,
            player.current_block_id.0,
            block_pos.x,
            block_pos.y,
            block_pos.z,
            havok.0,
            havok.1,
            havok.2,
            chr.block_id.0,
            chr.block_origin.0,
            chunk.0,
            chunk.1,
            chunk.2,
        ));
    }
    logger::log(&format!("player_chr_set: {count} player(s)."));

    let peers = sync::session_peers();
    for peer in &peers {
        logger::log(&format!(
            "Session: {} '{}'{}",
            peer.steam_id,
            peer.steam_name,
            if peer.is_local { " (local)" } else { "" }
        ));
    }
    let remote = sync::fresh_remote_spots();
    for (steam_id, r) in &remote {
        logger::log(&format!(
            "Synced: {steam_id} '{}' block {:#010X} local ({:.2}, {:.2}, {:.2}) - {:.1}s old",
            r.character_name,
            r.spot.block_id,
            r.spot.x,
            r.spot.y,
            r.spot.z,
            r.received.elapsed().as_secs_f32()
        ));
    }
    announce::show_announcement(&format!(
        "{count} loaded, {} in session, {} synced",
        peers.len(),
        remote.len()
    ));
}

/// Registers the per-frame hotkey watcher + position sync. Meant to run on
/// its own worker thread spawned from `DllMain`; never returns.
pub fn run() {
    let Some(fns) = resolve_game_fns() else {
        logger::error("Warp functions not found (AOB) - game update may need a mod update. SoulsTeleport disabled.");
        return;
    };
    logger::log(&format!(
        "Warp functions found (AOB), GameMan static at {:#X}.",
        fns.game_man_static as usize
    ));

    let cs_task = common::task::wait_for_cs_task();
    // Steam's messaging interface may not exist yet this early - retried
    // from the task until it does (logged once either way).
    let mut syncer: Option<sync::Syncer> = None;
    let mut steam_attempts = 0u32;
    common::task::run_recurring_safe(cs_task, "Teleport", CSTaskGroupIndex::FrameBegin, move |_data: &FD4TaskData| {
        if syncer.is_none() && steam_attempts < STEAM_LOAD_ATTEMPTS {
            steam_attempts += 1;
            if let Some(steam) = SteamMessages::load() {
                logger::log("Steam networking messages ready - position sync on.");
                syncer = Some(sync::Syncer::new(steam));
            } else if steam_attempts == STEAM_LOAD_ATTEMPTS {
                logger::error("Steam networking messages unavailable - position sync off.");
            }
        }
        if let Some(syncer) = syncer.as_mut() {
            syncer.tick();
        }

        let save_key = parse_virtual_key(&config::get_string("SaveKey", "F7"), VK_F7);
        let warp_key = parse_virtual_key(&config::get_string("WarpKey", "F8"), VK_F8);
        let list_key = parse_virtual_key(&config::get_string("ListPlayersKey", "F9"), VK_F9);
        let partner_key = parse_virtual_key(&config::get_string("WarpToPartnerKey", "F10"), VK_F10);
        if input::is_key_pressed(save_key) {
            save_current_spot();
        }
        if input::is_key_pressed(warp_key) {
            warp_to_saved(&fns);
        }
        if input::is_key_pressed(list_key) {
            list_players();
        }
        if input::is_key_pressed(partner_key) {
            warp_to_partner(&fns);
        }
    });

    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
