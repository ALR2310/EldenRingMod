//! "Warp to an exact position, with a loading screen" - used to land right
//! next to a co-op partner picked in the menu (see `ui`, `net`, README).
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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use eldenring::cs::CSTaskGroupIndex;
use eldenring::fd4::FD4TaskData;

// `common::input`, not fromsoftware-rs's `eldenring::util::input`: that one
// uses `GetKeyState`, which called from the game's task thread (no input
// queue of its own) reflects the whole desktop's key state - co-op test 4
// (2026-09-24): one hotkey press in 1 window made BOTH game instances warp.
// `common::input::is_key_pressed` checks that this process owns the
// foreground window before even polling.
use common::input::{self, parse_virtual_key};
use common::{config, logger, memscan};

use crate::net::{self, Net};
use crate::party;
use crate::ui;
use crate::steam::SteamMessages;

const VK_TAB: i32 = 0x09;

const MENU_REFRESH_EVERY: Duration = Duration::from_millis(500);

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

/// Address of the static holding the `GameMan*`, once resolved - lets
/// other modules ask [warp_pending] without a [GameFns].
static GAME_MAN_STATIC: AtomicUsize = AtomicUsize::new(0);

/// Whether a move-map (ours, or the game's own: fast travel, death...) is
/// requested and not yet picked up - i.e. a loading screen is imminent or
/// under way. `false` if GameMan isn't known/allocated yet.
pub fn warp_pending() -> bool {
    let static_addr = GAME_MAN_STATIC.load(Ordering::Relaxed);
    if static_addr == 0 {
        return false;
    }
    let game_man = unsafe { *(static_addr as *const usize) };
    game_man != 0 && unsafe { *((game_man + GAME_MAN_WARP_REQUESTED) as *const bool) }
}

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

/// Asks the game to move-map to `spot` (loading screen, then spawn exactly
/// there). Returns false if not in the world yet / GameMan not allocated.
pub fn warp_to(fns: &GameFns, spot: &Spot) -> bool {
    // Same "actually in the world" gate as every other mod here - warping
    // from a loading screen or the title menu is not something to test.
    if common::player::main_player_chr_ins_ptr().is_none() || warp_pending() {
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

/// Asks the partner the player clicked in the menu for their current
/// position; the warp happens once `HERE` arrives (see [handle_net_event]).
fn request_partner_position(net: &mut Net, steam_id: u64) {
    let members = party::members();
    let Some(own) = party::own_steam_id(&members) else {
        ui::set_status("Not in a co-op session.");
        return;
    };
    // Re-checked here, not just when the list was drawn: they may have left
    // the session since.
    let Some(target) = members.iter().find(|m| m.steam_id == steam_id && m.is_teleport_target()) else {
        ui::set_status("That player is no longer available.");
        return;
    };
    ui::with_shared(|s| {
        s.set_status(format!("Locating {}...", target.display_name()));
        s.busy = true;
    });
    net.request(own, target);
}

/// Refreshes the partner list the menu shows (only while it's open).
fn refresh_menu_snapshot() {
    let members = party::members();
    let in_session = party::own_steam_id(&members).is_some();
    let partners = members
        .iter()
        .filter(|m| m.is_teleport_target())
        .map(|m| ui::PartnerRow {
            steam_id: m.steam_id,
            character_name: m.display_name().to_string(),
            steam_name: m.steam_name.clone(),
            role: m.role(),
        })
        .collect();
    ui::with_shared(|s| {
        s.in_session = in_session;
        s.partners = partners;
    });
}

fn handle_net_event(fns: &GameFns, event: net::Event) {
    match event {
        net::Event::Arrived { character_name, spot } => {
            ui::with_shared(|s| s.busy = false);
            // No offset: the game itself pushes apart 2 characters spawned on
            // top of each other (confirmed by the user in-game).
            if warp_to(fns, &spot) {
                ui::set_status(format!("Teleporting to {character_name}..."));
                ui::MENU_OPEN.store(false, Ordering::Relaxed);
            } else {
                ui::set_status("Can't teleport right now (loading or not in the world).");
            }
        }
        net::Event::TimedOut { target_name } => {
            ui::with_shared(|s| {
                s.busy = false;
                s.set_status(format!("{target_name} did not respond."));
            });
        }
    }
}

/// Registers the per-frame hotkey watcher + position requests. Meant to run
/// on its own worker thread spawned from `DllMain`; never returns.
pub fn run() {
    let Some(fns) = resolve_game_fns() else {
        logger::error("Warp functions not found (AOB) - game update may need a mod update. SoulsTeleport disabled.");
        return;
    };
    logger::log(&format!(
        "Warp functions found (AOB), GameMan static at {:#X}.",
        fns.game_man_static as usize
    ));
    GAME_MAN_STATIC.store(fns.game_man_static as usize, Ordering::Relaxed);

    let cs_task = common::task::wait_for_cs_task();
    // Steam's messaging interface may not exist yet this early - retried
    // from the task until it does (logged once either way).
    let mut net: Option<Net> = None;
    let mut steam_attempts = 0u32;
    let mut last_snapshot: Option<Instant> = None;
    common::task::run_recurring_safe(cs_task, "Teleport", CSTaskGroupIndex::FrameBegin, move |_data: &FD4TaskData| {
        if net.is_none() && steam_attempts < STEAM_LOAD_ATTEMPTS {
            steam_attempts += 1;
            if let Some(steam) = SteamMessages::load() {
                logger::log("Steam networking messages ready - teleport to partner on.");
                net = Some(Net::new(steam));
            } else if steam_attempts == STEAM_LOAD_ATTEMPTS {
                logger::error("Steam networking messages unavailable - teleport to partner off.");
            }
        }
        if let Some(event) = net.as_mut().and_then(Net::tick) {
            handle_net_event(&fns, event);
        }

        let menu_key = parse_virtual_key(&config::get_string("MenuKey", "0x09"), VK_TAB);
        if input::is_key_pressed(menu_key) {
            ui::toggle();
        }
        if ui::MENU_OPEN.load(Ordering::Relaxed)
            && last_snapshot.is_none_or(|t: Instant| t.elapsed() >= MENU_REFRESH_EVERY)
        {
            last_snapshot = Some(Instant::now());
            refresh_menu_snapshot();
        }
        if let Some(steam_id) = ui::REQUEST.lock().unwrap().take() {
            match net.as_mut() {
                Some(net) => request_partner_position(net, steam_id),
                None => ui::set_status("Steam networking unavailable."),
            }
        }
    });

    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
