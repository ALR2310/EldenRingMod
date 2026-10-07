//! Keeps the game from reacting to the **mouse** while the menu is open (no
//! camera turning / attacking while clicking in the menu). Keyboard and
//! gamepad still reach the game, so the player can keep moving - user's
//! call (2026-09-25); the first version blocked all of them.
//!
//! The game doesn't read input from window messages (so `message_filter`
//! alone does nothing for it): its imports are `DINPUT8!DirectInput8Create`
//! (keyboard, mouse, DirectInput pads) and `XINPUT1_4` by ordinal (Xbox-style
//! pads). Same idea as QuestPath (its log mentions a hooked "dinput8
//! vtable"): hook the device read calls and, while the menu is open, still
//! call the original (keeps the game's device buffers drained) but hand the
//! game an idle state.
//!
//! - `IDirectInputDevice8::GetDeviceState` (vtable slot 9): zeroed only for
//!   the mouse formats (`DIMOUSESTATE`/`DIMOUSESTATE2`); the calling device is
//!   remembered as "a mouse".
//! - `IDirectInputDevice8::GetDeviceData` (slot 10): reports 0 buffered
//!   events, only for a device remembered as a mouse.
//! - XInput is no longer hooked (gamepads stay fully usable).
//!
//! The vtable is read off a throwaway keyboard device we create ourselves:
//! dinput8 uses one device implementation (one vtable per A/W flavor) for
//! every device type, so hooking the functions it points at covers the
//! game's devices too. Both flavors are hooked if they differ.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

use hudhook::mh::{MH_ApplyQueued, MhHook};

use crate::ui::MENU_OPEN;
use common::logger;

#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);

const IID_IDIRECTINPUT8W: Guid =
    Guid(0xBF798031, 0x483A, 0x4DA2, [0xAA, 0x99, 0x5D, 0x64, 0xED, 0x36, 0x97, 0x00]);
const IID_IDIRECTINPUT8A: Guid =
    Guid(0xBF798030, 0x483A, 0x4DA2, [0xAA, 0x99, 0x5D, 0x64, 0xED, 0x36, 0x97, 0x00]);
const GUID_SYS_KEYBOARD: Guid =
    Guid(0x6F1D2B61, 0xD5A0, 0x11CF, [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00]);
const DIRECTINPUT_VERSION: u32 = 0x0800;

const SLOT_RELEASE: usize = 2;
const SLOT_CREATE_DEVICE: usize = 3; // IDirectInput8
const SLOT_GET_DEVICE_STATE: usize = 9; // IDirectInputDevice8
const SLOT_GET_DEVICE_DATA: usize = 10;

const DIMOUSESTATE_SIZE: u32 = 16;
const DIMOUSESTATE2_SIZE: u32 = 20;


#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleA(name: *const u8) -> *mut c_void;
    fn LoadLibraryA(name: *const u8) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
}

type DirectInput8CreateFn =
    unsafe extern "system" fn(hinst: *mut c_void, version: u32, iid: *const Guid, out: *mut *mut c_void, outer: *mut c_void) -> i32;
type CreateDeviceFn =
    unsafe extern "system" fn(this: *mut c_void, guid: *const Guid, out: *mut *mut c_void, outer: *mut c_void) -> i32;
type ReleaseFn = unsafe extern "system" fn(this: *mut c_void) -> u32;
type GetDeviceStateFn = unsafe extern "system" fn(this: *mut c_void, size: u32, data: *mut c_void) -> i32;
type GetDeviceDataFn =
    unsafe extern "system" fn(this: *mut c_void, obj_size: u32, data: *mut c_void, in_out: *mut u32, flags: u32) -> i32;

// Up to 2 DirectInput flavors (W, A) - one trampoline slot each, keyed by the
// hooked function's address so a detour knows which original to call.
static STATE_TARGETS: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];
static STATE_ORIGS: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];
static DATA_ORIGS: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];
/// Device objects seen reading a mouse-format state (see [state_hook]).
static MOUSE_DEVICES: [AtomicUsize; 4] =
    [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)];

fn remember_mouse(this: *mut c_void) {
    let this = this as usize;
    if MOUSE_DEVICES.iter().any(|d| d.load(Ordering::Relaxed) == this) {
        return;
    }
    for slot in &MOUSE_DEVICES {
        if slot.compare_exchange(0, this, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
            return;
        }
    }
}

fn is_mouse(this: *mut c_void) -> bool {
    MOUSE_DEVICES.iter().any(|d| d.load(Ordering::Relaxed) == this as usize)
}

unsafe fn vtable_fn(obj: *mut c_void, slot: usize) -> usize {
    unsafe { *(*(obj as *const *const usize)).add(slot) }
}

fn menu_open() -> bool {
    MENU_OPEN.load(Ordering::Relaxed)
}

macro_rules! state_hook {
    ($name:ident, $i:expr) => {
        unsafe extern "system" fn $name(this: *mut c_void, size: u32, data: *mut c_void) -> i32 {
            let orig = unsafe { std::mem::transmute::<usize, GetDeviceStateFn>(STATE_ORIGS[$i].load(Ordering::Relaxed)) };
            let hr = unsafe { orig(this, size, data) };
            if matches!(size, DIMOUSESTATE_SIZE | DIMOUSESTATE2_SIZE) {
                remember_mouse(this);
                if hr >= 0 && menu_open() && !data.is_null() {
                    unsafe { std::ptr::write_bytes(data as *mut u8, 0, size as usize) };
                }
            }
            hr
        }
    };
}
state_hook!(get_device_state_hook_0, 0);
state_hook!(get_device_state_hook_1, 1);

macro_rules! data_hook {
    ($name:ident, $i:expr) => {
        unsafe extern "system" fn $name(
            this: *mut c_void,
            obj_size: u32,
            data: *mut c_void,
            in_out: *mut u32,
            flags: u32,
        ) -> i32 {
            let orig = unsafe { std::mem::transmute::<usize, GetDeviceDataFn>(DATA_ORIGS[$i].load(Ordering::Relaxed)) };
            let hr = unsafe { orig(this, obj_size, data, in_out, flags) };
            if hr >= 0 && menu_open() && !in_out.is_null() && is_mouse(this) {
                unsafe { *in_out = 0 };
            }
            hr
        }
    };
}
data_hook!(get_device_data_hook_0, 0);
data_hook!(get_device_data_hook_1, 1);

fn hook(target: usize, detour: *mut c_void, orig: &AtomicUsize) -> bool {
    if target == 0 {
        return false;
    }
    match unsafe { MhHook::new(target as *mut c_void, detour) } {
        Ok(h) => {
            orig.store(h.trampoline() as usize, Ordering::Relaxed);
            unsafe { h.queue_enable() }.is_ok()
        }
        Err(_) => false,
    }
}

/// Reads the device vtable's GetDeviceState/GetDeviceData for one
/// DirectInput flavor (W or A) off a throwaway keyboard device.
fn dinput_device_fns(create: DirectInput8CreateFn, iid: &Guid) -> Option<(usize, usize)> {
    unsafe {
        let hinst = GetModuleHandleA(std::ptr::null());
        let mut di: *mut c_void = std::ptr::null_mut();
        if create(hinst, DIRECTINPUT_VERSION, iid, &mut di, std::ptr::null_mut()) < 0 || di.is_null() {
            return None;
        }
        let create_device = std::mem::transmute::<usize, CreateDeviceFn>(vtable_fn(di, SLOT_CREATE_DEVICE));
        let mut dev: *mut c_void = std::ptr::null_mut();
        let fns = if create_device(di, &GUID_SYS_KEYBOARD, &mut dev, std::ptr::null_mut()) >= 0 && !dev.is_null() {
            let fns = (vtable_fn(dev, SLOT_GET_DEVICE_STATE), vtable_fn(dev, SLOT_GET_DEVICE_DATA));
            std::mem::transmute::<usize, ReleaseFn>(vtable_fn(dev, SLOT_RELEASE))(dev);
            Some(fns)
        } else {
            None
        };
        std::mem::transmute::<usize, ReleaseFn>(vtable_fn(di, SLOT_RELEASE))(di);
        fns
    }
}

fn install_dinput() -> usize {
    let module = unsafe { LoadLibraryA(c"dinput8.dll".as_ptr() as *const u8) };
    if module.is_null() {
        return 0;
    }
    let create = unsafe { GetProcAddress(module, c"DirectInput8Create".as_ptr() as *const u8) };
    if create.is_null() {
        return 0;
    }
    let create = unsafe { std::mem::transmute::<*mut c_void, DirectInput8CreateFn>(create) };

    let state_detours = [get_device_state_hook_0 as *mut c_void, get_device_state_hook_1 as *mut c_void];
    let data_detours = [get_device_data_hook_0 as *mut c_void, get_device_data_hook_1 as *mut c_void];
    let mut hooked = 0;
    for iid in [&IID_IDIRECTINPUT8W, &IID_IDIRECTINPUT8A] {
        let Some((state_fn, data_fn)) = dinput_device_fns(create, iid) else {
            continue;
        };
        // Same implementation for both flavors → already hooked.
        if STATE_TARGETS.iter().any(|t| t.load(Ordering::Relaxed) == state_fn) {
            continue;
        }
        let i = hooked;
        STATE_TARGETS[i].store(state_fn, Ordering::Relaxed);
        if hook(state_fn, state_detours[i], &STATE_ORIGS[i]) && hook(data_fn, data_detours[i], &DATA_ORIGS[i]) {
            hooked += 1;
        }
    }
    hooked
}

/// Installs the input hooks. MinHook must already be initialized (done by
/// `Hudhook::builder()` in `ui::install`).
pub fn install() {
    let dinput = install_dinput();
    let applied = unsafe { MH_ApplyQueued() }.ok().is_ok();
    if applied && dinput > 0 {
        logger::log(&format!("Menu: mouse blocking hooks applied (DirectInput flavors {dinput})."));
    } else {
        logger::error(&format!(
            "Menu: mouse blocking incomplete (DirectInput {dinput}, apply {applied}) - the camera may still turn while the menu is open."
        ));
    }
}
