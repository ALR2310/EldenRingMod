//! Inserts up to 3 new items into the Site of Grace menu, each toggled by
//! its own ini key: "Nâng cấp vũ khí" (`GraceMenu.Upgrade`, opens the
//! smithing/enhance menu via `OpenEnhanceShop`), "Mua" (`GraceMenu.Shop`,
//! opens `OpenRegularShop` with an item lot range covering every
//! purchasable item from every merchant in the game, combined into one
//! menu - see [shop_lot_range] for how that range is computed), and "Bán"
//! (`GraceMenu.Sell`, opens the sell-item menu via `OpenSellShop`).
//!
//! ## Backstory: the "player stands up" bug, and how it got fixed
//!
//! Every early attempt to run a shop-opening ESD command from a
//! custom-inserted Grace menu item made the player instantly stand up and
//! end the conversation, discarding whatever menu was about to open -
//! confirmed happening regardless of HOW the command was invoked (ESD
//! command embedded in a brand new runtime-allocated state,
//! `CS::CSEzStateTalkEvent`'s "fake talk session" dispatcher, or calling
//! the native open-menu function directly) and regardless of WHEN
//! (synchronously inside the `EzState::EnterState` hook, from a freshly
//! spawned thread, or from a spawned thread with a 50-500ms delay before
//! calling) - see `git stash@{0}` on this repo for that entire
//! investigation, preserved for reference but deliberately not restored
//! (this file was rewritten fresh from that point on).
//!
//! What eventually fixed it (confirmed in-game, 2026-08-28), after ruling
//! out "hijack a genuine vanilla state's `entry_events` in place, leaving
//! its `transitions` completely untouched" (still stood up): the SHOP
//! COMMAND'S OWN "menu is open" bookkeeping - `EldenConvenienceMod` (a
//! shipped, working, community-verified mod that statically compiles
//! equivalent entries into this exact ESD file) also rewrites that
//! state's own wait condition (`transitions[0].evaluator`) to
//! `CheckSpecificPersonMenuIsOpen(type, 0) == 0 || CheckSpecificPersonGenericDialogIsOpen(0)`
//! (`type` = the specific shop command's own menu-open tracking id - `5`
//! for `OpenRegularShop`; see `SoulsIds.ESDEdits.MenuCloseExpr`). Without
//! that rewrite, the hijacked state's ORIGINAL vanilla condition (tuned
//! for whatever menu type its own original content opened, not the one
//! we're now running) evaluates "closed" while the new shop's UI is still
//! initializing, leaving the ESD machine and the menu system out of sync
//! - which is what actually triggers the engine's "this talk is done,
//! stand the player up" logic. Rewriting the condition (while still
//! leaving `target_state`, the real return path, completely untouched)
//! fixed it - confirmed working end to end in-game.
//!
//! The above was confirmed by hijacking a genuine vanilla state
//! (`"Tailoring Shop"`, bank1 id 142) in place. That hijack has since been
//! FULLY REVERTED (Tailoring Shop is untouched, real vanilla behavior) -
//! there is no reason to believe `CheckSpecificPersonMenuIsOpen`'s
//! bookkeeping is keyed off which specific `EzState` object holds the
//! command rather than the live conversation/machine instance running it,
//! so [build_shop_state] builds a BRAND NEW state instead (never a
//! vanilla one - no existing Grace feature is repurposed or at risk of
//! ever being silently missing for some rare Grace instance). The new
//! state's single transition returns to [find_menu_rebuild_state]'s own
//! target - a genuine vanilla state, never modified, that vanilla itself
//! already treats as "menu closed, refresh the list" - so nothing about
//! the return path is invented either.
//!
//! ## Anchoring in content, not addresses
//!
//! Every Site of Grace runs its own copy of the same ESD state graph, at
//! a different address per instance and per game session - there is no
//! fixed address to hook. Instead, [is_grace_state_group] recognizes the
//! right graph by content: every Grace menu's state graph contains an
//! `add_talk_list_data` event listing message ID 15000395 ("Sort Chest"),
//! a menu entry present in every single Grace regardless of location
//! (technique borrowed from the open-source `erdGameTools` project, see
//! `.docs/Elden_Ring_game_tools`).
//!
//! ## `EzState::EnterState` hook
//!
//! Same observe-then-continue trampoline `regen/attack_hook.rs` uses:
//! patch the function's real 15-byte prologue, call a Rust callback with
//! the same 3 arguments the game passes in, then replay those exact 15
//! bytes (copied live, not hardcoded) before jumping back into the rest of
//! the real function - `EnterState` runs unmodified either way, this just
//! observes every call across every ESD machine in the game (not just
//! Grace's - the content check above discards everything else cheaply).
//!
//! Anchor pattern and prologue confirmed via Ghidra against the current
//! game build in the prior session (RVA `0x2088860`) - reused here as-is,
//! since this is a plain fact about the executable, not something this
//! rewrite has any reason to re-derive.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use eldenring::cs::{ShopLineupParam, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use common::codepatch;
use common::config;
use common::logger;
use common::memscan;

mod msg_hook;
mod unlock_shop;

// ===================== EzState (ESD) memory layout, from elden-x =====================

#[repr(C)]
struct EzSpan<T> {
    ptr: *mut T,
    len: usize,
}

impl<T: 'static> EzSpan<T> {
    /// # Safety
    /// `self` must point at real, valid data (live game memory, or a
    /// buffer this module itself leaked permanently).
    unsafe fn as_slice(&self) -> &'static [T] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    fn from_vec(v: Vec<T>) -> Self {
        let leaked: &'static mut [T] = Box::leak(v.into_boxed_slice());
        Self { ptr: leaked.as_mut_ptr(), len: leaked.len() }
    }
}

// Not `#[derive(Clone, Copy)]` - derive wrongly infers a `T: Copy` bound
// (`*mut T` is always Copy regardless of `T`).
impl<T> Clone for EzSpan<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for EzSpan<T> {}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct EzCommand {
    bank: i32,
    id: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EzEvent {
    command: EzCommand,
    args: EzSpan<EzSpan<u8>>,
}

#[repr(C)]
struct EzTransition {
    target_state: *mut EzState,
    pass_events: EzSpan<EzEvent>,
    sub_transitions: EzSpan<*mut EzTransition>,
    evaluator: EzSpan<u8>,
}

#[repr(C)]
struct EzState {
    id: i32,
    transitions: EzSpan<*mut EzTransition>,
    entry_events: EzSpan<EzEvent>,
    exit_events: EzSpan<EzEvent>,
    while_events: EzSpan<EzEvent>,
}

#[repr(C)]
struct EzStateGroup {
    id: i32,
    states: EzSpan<EzState>,
    initial_state: *mut EzState,
}

// elden-x's `machine` has a `virtual ~machine()`, so the real object has a
// vtable pointer at offset 0 that isn't visible in that header's own
// field list but still occupies the first 8 bytes in memory.
#[repr(C)]
struct EzMachine {
    _vtable: usize,
    _unk1: [u8; 0x20],
    state_group: *mut EzStateGroup,
    _unk2: [u8; 0x110],
}

// ===================== relevant ESD commands (elden-x/talk_commands.hpp) =====================

const CMD_ADD_TALK_LIST_DATA: EzCommand = EzCommand { bank: 1, id: 19 };
const CMD_ADD_TALK_LIST_DATA_IF: EzCommand = EzCommand { bank: 5, id: 19 };
const CMD_ADD_TALK_LIST_DATA_ALT: EzCommand = EzCommand { bank: 5, id: 149 };
const CMD_OPEN_REPOSITORY: EzCommand = EzCommand { bank: 1, id: 30 };
const CMD_OPEN_REGULAR_SHOP: EzCommand = EzCommand { bank: 1, id: 22 };
const CMD_CLEAR_TALK_LIST_DATA: EzCommand = EzCommand { bank: 1, id: 20 };
const CMD_COMBINE_MENU_FLAG_AND_EVENT_FLAG: EzCommand = EzCommand { bank: 1, id: 49 };
const CMD_OPEN_ENHANCE_SHOP: EzCommand = EzCommand { bank: 1, id: 24 };
const CMD_OPEN_SELL_SHOP: EzCommand = EzCommand { bank: 1, id: 46 };
// Purpose unconfirmed (community CT-TGA project labels it
// "Unknown141_PlaylogRelated") - included anyway, verbatim, because
// EldenConvenienceMod's own known-working Grace-Upgrade entry calls it
// right before OpenEnhanceShop with the same argument (9) used below as
// the wait-condition's menu type. Zero-cost to include even if it turns
// out to be irrelevant telemetry.
const CMD_UNKNOWN_141: EzCommand = EzCommand { bank: 1, id: 141 };

// "Sort Chest" - present in every Site of Grace's menu, used purely as a
// content fingerprint to recognize the right state_group (see module doc
// comment), independent of address.
const SORT_CHEST_MSG_ID: i32 = 15000395;

// Fallback item lot range, only used if [shop_lot_range] can't read
// `SoloParamRepository` for some reason - the exact range the community's
// own "All Shops" Cheat Engine table script uses
// (`executeEzStateEvent(EzStateEvent.OpenRegularShop, {0, 9999999})`,
// `Elden-Ring-CT-TGA`'s cheat table). An earlier version of this file used
// this fixed range unconditionally; see [shop_lot_range]'s own doc comment
// for why that caused a visible hitch opening "Mua".
const ALL_SHOPS_LOT_START: i32 = 0;
const ALL_SHOPS_LOT_END: i32 = 9999999;

/// Real min/max `ShopLineupParam` row IDs, computed once (the first time
/// "Mua" is built, i.e. the first Site of Grace visited this session) and
/// cached for the rest of the process's life - replaces the fixed
/// `ALL_SHOPS_LOT_START..ALL_SHOPS_LOT_END` range every earlier version of
/// this file passed to `OpenRegularShop` unconditionally.
///
/// This narrows the range but does NOT fix the ~1-2s hitch opening "Mua"
/// (confirmed in-game, 2026-08-29) - a one-off diagnostic dump (per
/// `equip_type` row-ID span) showed real rows are scattered across nearly
/// the entire `0..9999999` space regardless of item category, so the
/// actual cost is proportional to the ~1300 real rows `OpenRegularShop`
/// has to build a lineup for, not to how wide a numeric range surrounds
/// them - narrowing only trims dead air, which turned out to be
/// negligible next to that real per-item cost. A genuinely hitch-free
/// combined shop would need to bypass `OpenRegularShop` entirely (bespoke
/// UI list construction, or ESD dialogue-list-based item selection like
/// `ermerchant.dll` appears to use) - out of scope for now; kept as a
/// harmless narrowing rather than reverted, and falls back to the full
/// range if `SoloParamRepository` isn't available yet or somehow has zero
/// rows (should not happen in practice - the Grace menu itself only
/// exists once the player is in the game world, well after params load).
fn shop_lot_range() -> (i32, i32) {
    static RANGE: OnceLock<(i32, i32)> = OnceLock::new();
    *RANGE.get_or_init(|| {
        let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else {
            logger::warn("GraceMenu: SoloParamRepository not available, falling back to full shop lot range.");
            return (ALL_SHOPS_LOT_START, ALL_SHOPS_LOT_END);
        };

        let (mut min, mut max) = (i32::MAX, i32::MIN);
        for (id, _) in repo.rows::<ShopLineupParam>() {
            let id = id as i32;
            min = min.min(id);
            max = max.max(id);
        }

        if min > max {
            logger::warn("GraceMenu: ShopLineupParam has no rows, falling back to full shop lot range.");
            return (ALL_SHOPS_LOT_START, ALL_SHOPS_LOT_END);
        }

        logger::log(&format!(
            "GraceMenu: narrowed shop lot range to {min}..{max} (was {ALL_SHOPS_LOT_START}..{ALL_SHOPS_LOT_END})."
        ));
        (min, max)
    })
}

// Message IDs for the new items' displayed text - `msg_hook` hooks the
// game's own text lookup to return custom text for these specific IDs
// (never real FMG entries) instead of the borrowed vanilla IDs an
// earlier version of this file used.
use msg_hook::{SELL_MSG_ID, SHOP_MSG_ID, UPGRADE_MSG_ID};

// Talk-list indices for the new items - chosen not to collide with any
// vanilla item (erdGameTools uses 70 for its own equivalent test item;
// this picks the next ones, same reasoning).
const UPGRADE_OPTION_INDEX: i32 = 71;
const SHOP_OPTION_INDEX: i32 = 72;
const SELL_OPTION_INDEX: i32 = 73;

fn make_int_expression(value: i32) -> [u8; 6] {
    let b = value.to_le_bytes();
    [0x82, b[0], b[1], b[2], b[3], 0xa1]
}

fn make_talk_list_result_expression(value: i32) -> [u8; 9] {
    let b = value.to_le_bytes();
    [0x57, 0x84, 0x82, b[0], b[1], b[2], b[3], 0x95, 0xa1]
}

/// `!(CheckSpecificPersonMenuIsOpen(menu_type, 0) == 1 && CheckSpecificPersonGenericDialogIsOpen(0) == 0)`
/// - i.e. "this menu type is no longer open, OR a generic dialog is
/// currently open" - the exact wait condition
/// `EldenConvenienceMod`/`SoulsIds.ESDEdits.MenuCloseExpr(menu_type)` puts
/// on the state it repurposes for its own Grace entries (`5` for
/// `OpenRegularShop`, `6` for `OpenSellShop`, `9` for `OpenEnhanceShop`).
///
/// Byte encoding derived from the game's own ESD opcode table (function id
/// N in -64..=63 -> single byte N+64, terminated by 0x84+argCount; int
/// literal in the same range -> single byte value+64; "==" -> 0x95, "&&"
/// -> 0x98; every top-level expression ends with 0xA1) - confirmed correct
/// (2026-08-28) by first decoding a known-working reference expression
/// (`MenuCloseExpr(1)`) byte-for-byte with this exact scheme and
/// reproducing precisely `ESDEdits.cs`'s own documented comment for it,
/// then re-deriving the same expression at menu type 9 (`OpenEnhanceShop`)
/// from scratch and confirming in-game it fixed the stand-up bug for that
/// command - every menu type only ever differs by byte 2 (`menu_type + 64`).
///
/// f59(t,0)=[0x7b,t+64,0x40,0x86]; ==1=[0x41,0x95]; f58(0)=[0x7a,0x40,0x85];
/// ==0=[0x40,0x95]; &&=[0x98]; ==0=[0x40,0x95]; end=[0xa1]
fn make_menu_closed_expression(menu_type: i32) -> [u8; 15] {
    let t = (menu_type + 64) as u8;
    [0x7b, t, 0x40, 0x86, 0x41, 0x95, 0x7a, 0x40, 0x85, 0x40, 0x95, 0x98, 0x40, 0x95, 0xa1]
}

unsafe fn get_ezstate_int_value(expr: &EzSpan<u8>) -> Option<i32> {
    let bytes = unsafe { expr.as_slice() };
    match bytes.len() {
        2 => Some(bytes[0] as i32 - 64),
        6 if bytes[0] == 0x82 => Some(i32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]])),
        _ => None,
    }
}

unsafe fn is_sort_chest_event(event: &EzEvent) -> bool {
    let args = unsafe { event.args.as_slice() };
    if event.command == CMD_ADD_TALK_LIST_DATA {
        args.get(1).is_some_and(|a| unsafe { get_ezstate_int_value(a) } == Some(SORT_CHEST_MSG_ID))
    } else if event.command == CMD_ADD_TALK_LIST_DATA_IF || event.command == CMD_ADD_TALK_LIST_DATA_ALT {
        args.get(2).is_some_and(|a| unsafe { get_ezstate_int_value(a) } == Some(SORT_CHEST_MSG_ID))
    } else {
        false
    }
}

unsafe fn is_grace_state_group(group: *const EzStateGroup) -> bool {
    if group.is_null() {
        return false;
    }
    let states = unsafe { (*group).states.as_slice() };
    states.iter().any(|state| unsafe { state.entry_events.as_slice() }.iter().any(|e| unsafe { is_sort_chest_event(e) }))
}

/// Whether `group` (THIS specific live instance - every Site of Grace
/// runs its own fresh copy of the ESD graph at a different address every
/// time it's entered, see module doc comment) already has our own
/// inserted items. Content-based, not a global "have we ever patched
/// anything" flag: a global flag would (and, in an earlier version of
/// this file, did) patch only the very first Grace ever entered for the
/// whole session, since every other Grace - and even the SAME Grace
/// re-entered later - gets a brand new graph instance in memory that
/// was never itself patched. Checked instead of a global flag before
/// every insertion attempt, so a fresh, unpatched instance always gets
/// patched exactly once, no matter how many Graces (or re-visits) happen
/// in a session.
unsafe fn already_has_custom_items(group: *const EzStateGroup) -> bool {
    if group.is_null() {
        return false;
    }
    let states = unsafe { (*group).states.as_slice() };
    states.iter().any(|state| {
        unsafe { state.entry_events.as_slice() }.iter().any(|e| {
            e.command == CMD_ADD_TALK_LIST_DATA
                && unsafe { e.args.as_slice() }.get(1).is_some_and(|a| {
                    matches!(unsafe { get_ezstate_int_value(a) }, Some(UPGRADE_MSG_ID) | Some(SHOP_MSG_ID) | Some(SELL_MSG_ID))
                })
        })
    })
}

unsafe fn targets_open_repository(transition: *const EzTransition) -> bool {
    if transition.is_null() {
        return false;
    }
    let target = unsafe { (*transition).target_state };
    if target.is_null() {
        return false;
    }
    unsafe { (*target).entry_events.as_slice() }.first().is_some_and(|e| e.command == CMD_OPEN_REPOSITORY)
}

/// Finds the state that "rebuilds the main menu list" (`entry_events` is
/// exactly one argument-less `clear_talk_list_data`) - a genuine vanilla
/// state, present in every Grace, that vanilla itself already transitions
/// back into after any menu closes. Used purely as a safe RETURN target
/// for [build_shop_state]/[build_sell_state]/[build_upgrade_state]'s own
/// transition - never modified.
unsafe fn find_menu_rebuild_state(group: *const EzStateGroup) -> Option<*mut EzState> {
    if group.is_null() {
        return None;
    }
    let group_ref = unsafe { &*group };
    let states = unsafe { group_ref.states.as_slice() };
    for i in 0..states.len() {
        let state_ptr = unsafe { group_ref.states.ptr.add(i) };
        let state = unsafe { &*state_ptr };
        let entry_events = unsafe { state.entry_events.as_slice() };
        if entry_events.len() == 1 && entry_events[0].command == CMD_CLEAR_TALK_LIST_DATA && unsafe { entry_events[0].args.as_slice() }.is_empty()
        {
            return Some(state_ptr);
        }
    }
    None
}

/// Builds a BRAND NEW state (never a vanilla one - no existing Grace
/// feature is touched or repurposed) whose `entry_events` run `commands`
/// verbatim, whose single `transitions[0]` has `evaluator` =
/// `make_menu_closed_expression(menu_type)` (matching whichever shop
/// command in `commands` actually opens a menu) and `target_state` =
/// `return_target` (a genuine vanilla state, see [find_menu_rebuild_state]
/// - never a state this module invented).
///
/// This mirrors the exact `entry_events` + `transitions[0].evaluator`
/// shape confirmed to fix the "player stands up" bug (see module doc
/// comment) - the only difference from that confirmed-working version is
/// that the state hosting them is freshly allocated here instead of a
/// hijacked vanilla one, since there is no reason to believe
/// `CheckSpecificPersonMenuIsOpen`'s bookkeeping is keyed off which
/// specific `EzState` object holds the command rather than the live
/// conversation/machine instance running it.
fn build_action_state(commands: &[(EzCommand, &[i32])], menu_type: i32, return_target: *mut EzState) -> *mut EzState {
    let new_events: Vec<EzEvent> = commands
        .iter()
        .map(|(cmd, args)| {
            let arg_spans: Vec<EzSpan<u8>> = args
                .iter()
                .map(|&v| {
                    let bytes = Box::leak(Box::new(make_int_expression(v)));
                    EzSpan { ptr: bytes.as_mut_ptr(), len: bytes.len() }
                })
                .collect();
            EzEvent { command: *cmd, args: EzSpan::from_vec(arg_spans) }
        })
        .collect();

    let cond_bytes = Box::leak(Box::new(make_menu_closed_expression(menu_type)));
    let transition = Box::leak(Box::new(EzTransition {
        target_state: return_target,
        pass_events: EzSpan::empty(),
        sub_transitions: EzSpan::empty(),
        evaluator: EzSpan { ptr: cond_bytes.as_mut_ptr(), len: cond_bytes.len() },
    }));

    Box::leak(Box::new(EzState {
        id: 0,
        transitions: EzSpan::from_vec(vec![transition as *mut EzTransition]),
        entry_events: EzSpan::from_vec(new_events),
        exit_events: EzSpan::empty(),
        while_events: EzSpan::empty(),
    })) as *mut EzState
}

/// Opens `OpenRegularShop` against [shop_lot_range] (menu type 5). See
/// [build_action_state].
fn build_shop_state(return_target: *mut EzState) -> *mut EzState {
    let (start, end) = shop_lot_range();
    build_action_state(&[(CMD_OPEN_REGULAR_SHOP, &[start, end])], 5, return_target)
}

/// Opens `OpenSellShop` (menu type 6) - `(-1, -1)` copied verbatim from
/// `EldenConvenienceMod`'s own known-working sequence for this exact
/// command (`c1_46 OpenSellShop(-1, -1)`). See [build_action_state].
fn build_sell_state(return_target: *mut EzState) -> *mut EzState {
    build_action_state(&[(CMD_OPEN_SELL_SHOP, &[-1, -1])], 6, return_target)
}

/// Opens `OpenEnhanceShop` (menu type 9) - the 4
/// `CombineMenuFlagAndEventFlag` calls plus [CMD_UNKNOWN_141] are copied
/// verbatim from `EldenConvenienceMod`'s own known-working sequence for
/// this exact command (see [CMD_UNKNOWN_141]'s doc comment). See
/// [build_action_state].
fn build_upgrade_state(return_target: *mut EzState) -> *mut EzState {
    build_action_state(
        &[
            (CMD_COMBINE_MENU_FLAG_AND_EVENT_FLAG, &[6001, 232]),
            (CMD_COMBINE_MENU_FLAG_AND_EVENT_FLAG, &[6001, 233]),
            (CMD_COMBINE_MENU_FLAG_AND_EVENT_FLAG, &[6001, 234]),
            (CMD_COMBINE_MENU_FLAG_AND_EVENT_FLAG, &[6001, 235]),
            (CMD_UNKNOWN_141, &[9]),
            (CMD_OPEN_ENHANCE_SHOP, &[0]),
        ],
        9,
        return_target,
    )
}

/// Builds one new top-level menu item's insertion pieces: 1 event that
/// lists it in the main talk-list, 1 transition that fires when it's
/// picked. Both get leaked into the returned addresses so they survive
/// past this call - [patch_state_group] copies these pointers into the
/// game's own (also leaked) arrays.
struct MenuItemInsertion {
    add_item_event: EzEvent,
    option_transition_addr: usize,
}

fn build_menu_item_insertion(message_id: i32, option_index: i32, target_state: *mut EzState) -> MenuItemInsertion {
    let index_bytes = Box::leak(Box::new(make_int_expression(option_index)));
    let message_bytes = Box::leak(Box::new(make_int_expression(message_id)));
    let placeholder_bytes = Box::leak(Box::new(make_int_expression(-1)));
    let add_item_event = EzEvent {
        command: CMD_ADD_TALK_LIST_DATA,
        args: EzSpan::from_vec(vec![
            EzSpan { ptr: index_bytes.as_mut_ptr(), len: index_bytes.len() },
            EzSpan { ptr: message_bytes.as_mut_ptr(), len: message_bytes.len() },
            EzSpan { ptr: placeholder_bytes.as_mut_ptr(), len: placeholder_bytes.len() },
        ]),
    };

    let cond_bytes = Box::leak(Box::new(make_talk_list_result_expression(option_index)));
    let transition = Box::leak(Box::new(EzTransition {
        target_state,
        pass_events: EzSpan::empty(),
        sub_transitions: EzSpan::empty(),
        evaluator: EzSpan { ptr: cond_bytes.as_mut_ptr(), len: cond_bytes.len() },
    }));

    MenuItemInsertion { add_item_event, option_transition_addr: transition as *mut EzTransition as usize }
}

impl<T> EzSpan<T> {
    fn empty() -> Self {
        Self { ptr: std::ptr::null_mut(), len: 0 }
    }
}

/// Inserts every item in `insertions` into the live Grace menu in one
/// pass: finds the same 2 content-identified anchor points `erdGameTools`
/// uses (the state that lists "Sort Chest", to append each new
/// `add_talk_list_data` event; the state whose transitions include one to
/// `OpenRepository`, to insert each new selection transition right after
/// it, in `insertions`' own order), then splices both in by building a
/// new backing array and reassigning the span - confirmed safe in the
/// prior session (this exact insertion mechanism was never what caused
/// the stand-up bug there).
unsafe fn patch_state_group(group: *mut EzStateGroup, insertions: &[MenuItemInsertion]) -> bool {
    let group_ref = unsafe { &*group };
    let states = unsafe { group_ref.states.as_slice() };

    let mut add_item_state: *mut EzState = std::ptr::null_mut();
    let mut transition_state: *mut EzState = std::ptr::null_mut();
    let mut transition_index: isize = -1;

    for i in 0..states.len() {
        let state_ptr = unsafe { group_ref.states.ptr.add(i) };
        let state = unsafe { &*state_ptr };

        if unsafe { state.entry_events.as_slice() }.iter().any(|e| unsafe { is_sort_chest_event(e) }) {
            add_item_state = state_ptr;
        }

        let transitions = unsafe { state.transitions.as_slice() };
        for (j, &t) in transitions.iter().enumerate() {
            if unsafe { targets_open_repository(t) } {
                transition_state = state_ptr;
                transition_index = j as isize;
                break;
            }
        }
    }

    if add_item_state.is_null() || transition_state.is_null() || transition_index < 0 {
        logger::warn("GraceMenu: anchor states not found (Sort Chest event / OpenRepository transition) - skipped this entry, will retry next time Grace is entered.");
        return false;
    }

    let add_item_state_ref = unsafe { &mut *add_item_state };
    let mut new_events: Vec<EzEvent> = unsafe { add_item_state_ref.entry_events.as_slice() }.to_vec();
    new_events.extend(insertions.iter().map(|i| i.add_item_event));
    add_item_state_ref.entry_events = EzSpan::from_vec(new_events);

    let transition_state_ref = unsafe { &mut *transition_state };
    let existing: Vec<*mut EzTransition> = unsafe { transition_state_ref.transitions.as_slice() }.to_vec();
    let insert_at = transition_index as usize + 1;
    let mut new_transitions = Vec::with_capacity(existing.len() + insertions.len());
    new_transitions.extend_from_slice(&existing[..insert_at]);
    new_transitions.extend(insertions.iter().map(|i| i.option_transition_addr as *mut EzTransition));
    new_transitions.extend_from_slice(&existing[insert_at..]);
    transition_state_ref.transitions = EzSpan::from_vec(new_transitions);

    logger::log(&format!("GraceMenu: inserted {} new item(s) into the Site of Grace menu.", insertions.len()));
    true
}

/// Called from the trampoline (see below) on every `EzState::EnterState`
/// call in the game, for every ESD machine - cheap to discard (content
/// check only) for the overwhelming majority that aren't Grace.
unsafe fn on_enter_state(state: *mut EzState, machine: *mut EzMachine) {
    if state.is_null() || machine.is_null() {
        return;
    }
    let group = unsafe { (*machine).state_group };
    if !unsafe { is_grace_state_group(group) } {
        return;
    }
    let group_ref = unsafe { &*group };
    if state != group_ref.initial_state {
        return;
    }
    // This SPECIFIC instance's own graph, not "have we ever patched
    // anything" - see [already_has_custom_items]'s doc comment for why a
    // global flag broke every Grace but the first one ever visited.
    if unsafe { already_has_custom_items(group) } {
        return;
    }

    let Some(return_target) = (unsafe { find_menu_rebuild_state(group) }) else {
        logger::warn("GraceMenu: menu_rebuild_state (clear_talk_list_data) not found in this Grace's graph - skipped this entry, will retry next time Grace is entered.");
        return;
    };

    let mut insertions = Vec::with_capacity(3);
    if config::get_bool("GraceMenu.Upgrade", true) {
        let upgrade_state = build_upgrade_state(return_target);
        insertions.push(build_menu_item_insertion(UPGRADE_MSG_ID, UPGRADE_OPTION_INDEX, upgrade_state));
    }
    if config::get_bool("GraceMenu.Shop", true) {
        let shop_state = build_shop_state(return_target);
        insertions.push(build_menu_item_insertion(SHOP_MSG_ID, SHOP_OPTION_INDEX, shop_state));
    }
    if config::get_bool("GraceMenu.Sell", true) {
        let sell_state = build_sell_state(return_target);
        insertions.push(build_menu_item_insertion(SELL_MSG_ID, SELL_OPTION_INDEX, sell_state));
    }
    if insertions.is_empty() {
        return;
    }
    logger::log(&format!(
        "GraceMenu: built {} new state(s), all returning to genuine vanilla state {return_target:p} - no vanilla feature touched.",
        insertions.len()
    ));

    unsafe { patch_state_group(group, &insertions) };
}

#[unsafe(no_mangle)]
extern "C" fn ezstate_enter_state_observed(state: *mut EzState, machine: *mut EzMachine, _unk: *mut std::ffi::c_void) {
    let _ = std::panic::catch_unwind(|| unsafe { on_enter_state(state, machine) });
}

// ===================== EzState::EnterState hook =====================

const ENTER_STATE_ANCHOR_PATTERN: &str = "80 7e 18 00 74 15 4c 8d 44 24 40 48 8b d6 48 8b 4e 20 e8 ?? ?? ?? ??";
const ENTER_STATE_CALL_OFFSET: usize = 18;

// EnterState's real 15-byte prologue (mov rax,rsp; push rdi; push r14;
// push r15; sub rsp,0x420), confirmed via Ghidra against the current game
// build (RVA 0x2088860) in the prior investigation session - replayed
// verbatim by the trampoline below after the observer callback runs, so
// the real function continues completely unmodified.
const ENTER_STATE_PROLOGUE_LEN: usize = 15;

#[unsafe(no_mangle)]
static ENTER_STATE_RETURN_ADDR: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" {
    fn ezstate_enter_state_trampoline();
}

std::arch::global_asm!(
    r#"
.global ezstate_enter_state_trampoline
ezstate_enter_state_trampoline:
    push    rcx
    push    rdx
    push    r8
    sub     rsp, 0x20
    call    ezstate_enter_state_observed
    add     rsp, 0x20
    pop     r8
    pop     rdx
    pop     rcx
    mov     rax, rsp
    push    rdi
    push    r14
    push    r15
    sub     rsp, 0x420
    mov     r11, qword ptr [rip + ENTER_STATE_RETURN_ADDR]
    jmp     r11
"#
);

fn install() -> bool {
    let Some(anchor) = memscan::wait_for_pattern_in_module(ENTER_STATE_ANCHOR_PATTERN, Duration::from_millis(500), Duration::from_secs(60))
    else {
        logger::error("GraceMenu: EnterState anchor pattern not found within the timeout. Game may have been updated - re-check ENTER_STATE_ANCHOR_PATTERN.");
        return false;
    };

    let call_site = unsafe { anchor.add(ENTER_STATE_CALL_OFFSET) };
    if unsafe { *call_site } != 0xE8 {
        logger::error("GraceMenu: byte at the expected CALL site isn't 0xE8 (layout differs from expected), disabled.");
        return false;
    }
    let rel32 = unsafe { i32::from_le_bytes(*(call_site.add(1) as *const [u8; 4])) };
    let enter_state = unsafe { call_site.add(5).offset(rel32 as isize) };

    ENTER_STATE_RETURN_ADDR.store(enter_state as usize + ENTER_STATE_PROLOGUE_LEN, Ordering::Relaxed);

    let mut patch = [0x90u8; ENTER_STATE_PROLOGUE_LEN]; // NOP-fill the remainder
    patch[0] = 0x48;
    patch[1] = 0xB8; // mov rax, imm64
    let trampoline_addr = ezstate_enter_state_trampoline as *const () as u64;
    patch[2..10].copy_from_slice(&trampoline_addr.to_le_bytes());
    patch[10] = 0xFF;
    patch[11] = 0xE0; // jmp rax

    if !unsafe { codepatch::overwrite_bytes(enter_state, &patch) } {
        logger::error("GraceMenu: VirtualProtect/patch on EnterState failed, disabled.");
        return false;
    }

    logger::log(&format!("GraceMenu: hooked EzState::EnterState at {enter_state:p}."));
    true
}

/// Installs the hook once (retrying the AOB scan for up to 60s), then
/// parks - all the real logic runs inside the hook itself
/// (`on_enter_state`), there is no per-frame tick of its own here. Meant
/// to run on its own worker thread spawned from `DllMain`; never returns
/// (except early, if the hook fails to install).
pub fn run() {
    if !config::get_bool("GraceMenu.Enabled", true) {
        logger::log("GraceMenu.Enabled=false - skipping entirely at startup.");
        return;
    }

    // Not fatal if this fails (logged, custom text just falls back to
    // whatever the real FMG entry for these IDs would be - none, so the
    // items would show blank/placeholder text rather than break anything)
    // - install the EnterState hook regardless.
    msg_hook::install();

    // Independent param edit, own toggle (`GraceMenu.UnlockShop`)
    // - can take a while waiting for `SoloParamRepository`, so runs on its
    // own thread instead of delaying the EnterState hook below.
    std::thread::spawn(unlock_shop::run);

    if !install() {
        logger::warn("GraceMenu: disabled for this session (hook install failed).");
        return;
    }

    logger::log("GraceMenu: waiting to detect a Site of Grace...");
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}
