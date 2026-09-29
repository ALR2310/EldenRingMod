//! Makes the game's windowed-mode window resizable by dragging its edges.
//!
//! The game creates its window without `WS_THICKFRAME`, so Windows gives it
//! no resize border at all - its smallest in-game resolution (800x450) is
//! also the smallest the window ever gets. An outside `SetWindowPos` test
//! (2026-09-29) showed the game keeps rendering the full scene at any window
//! size (DXGI scales the swapchain into the client area), so nothing inside
//! the game needs patching: adding the style bit back is enough for Windows
//! to handle the drag itself.
//!
//! Two things still need the window procedure subclassed:
//! - `WM_SETCURSOR`: the game forces its own cursor for every hit-test
//!   code, so the resize arrows never showed even though the border worked
//!   (hit-test on the edges already returned HTLEFT/HTBOTTOMRIGHT/...).
//!   Sizing codes are handed to `DefWindowProcW` instead.
//! - `WM_GETMINMAXINFO`/`WM_SIZING`: the `MinWidth`/`MinHeight` floor and
//!   the optional `AspectWidth`:`AspectHeight` lock, both measured on the
//!   client area (the game image), not the outer window.
//!
//! The style bit is (re)applied from a 1s watchdog rather than once: the
//! game rewrites its window style whenever the screen mode or resolution
//! changes in its settings, which would silently drop the border again.

use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use common::{config, logger};

type Hwnd = *mut c_void;

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[allow(dead_code)] // only written, read by Windows
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[allow(dead_code)] // only written, read by Windows
struct MinMaxInfo {
    reserved: Point,
    max_size: Point,
    max_position: Point,
    min_track_size: Point,
    max_track_size: Point,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(callback: unsafe extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    fn IsWindow(hwnd: Hwnd) -> i32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn IsIconic(hwnd: Hwnd) -> i32;
    fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    fn SetThreadDpiAwarenessContext(context: isize) -> isize;
    fn GetClassNameW(hwnd: Hwnd, buf: *mut u16, len: i32) -> i32;
    fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    fn SetWindowPos(hwnd: Hwnd, after: Hwnd, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn CallWindowProcW(prev: isize, hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn AdjustWindowRectExForDpi(rect: *mut Rect, style: u32, menu: i32, ex_style: u32, dpi: u32) -> i32;
    fn GetDpiForWindow(hwnd: Hwnd) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcessId() -> u32;
}

const GWLP_WNDPROC: i32 = -4;
const GWL_STYLE: i32 = -16;
const GWL_EXSTYLE: i32 = -20;

const WS_CAPTION: u32 = 0x00C0_0000;
const WS_THICKFRAME: u32 = 0x0004_0000;

const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;

const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;

const WM_SETCURSOR: u32 = 0x0020;
const WM_GETMINMAXINFO: u32 = 0x0024;
const WM_SIZING: u32 = 0x0214;
const WM_ENTERSIZEMOVE: u32 = 0x0231;

// Hit-test codes of the resize border, HTLEFT..=HTBOTTOMRIGHT.
const HT_SIZE_FIRST: u32 = 10;
const HT_SIZE_LAST: u32 = 17;

// WM_SIZING wParam: which edge/corner is being dragged.
const WMSZ_LEFT: usize = 1;
const WMSZ_TOP: usize = 3;
const WMSZ_TOPLEFT: usize = 4;
const WMSZ_TOPRIGHT: usize = 5;
const WMSZ_BOTTOM: usize = 6;
const WMSZ_BOTTOMLEFT: usize = 7;

/// The game's own window procedure, called for everything we don't handle.
static ORIG_WNDPROC: AtomicIsize = AtomicIsize::new(0);
/// Whether the resize border is currently on because we added it - the
/// subclassed messages only do anything while it is.
static FRAME_ADDED: AtomicBool = AtomicBool::new(false);
/// Set once the player starts dragging the border - from then on the
/// startup `Width`/`Height` is never re-applied over their size.
static USER_RESIZED: AtomicBool = AtomicBool::new(false);

/// How long after hooking the startup size keeps being re-applied: the game
/// may still set its own resolution on the window right after creating it.
const START_SIZE_WINDOW: Duration = Duration::from_secs(20);

/// Hooks the game window once it exists, then keeps its resize border in
/// sync with the ini for the rest of the process's life. Never returns.
pub fn run() {
    // Physical pixels for this thread's own calls (`Width`/`Height`,
    // `GetClientRect`/`SetWindowPos`), whatever DPI mode the game runs in -
    // a DPI-unaware caller gets scaled coordinates (a 480x300 request came out
    // 720x450 at 150% in the first outside test).
    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let mut hooked: Hwnd = null_mut();
    let mut hooked_at = Instant::now();
    loop {
        if hooked.is_null() || unsafe { IsWindow(hooked) } == 0 {
            hooked = null_mut();
            if let Some(hwnd) = find_game_window() {
                hook(hwnd);
                hooked = hwnd;
                hooked_at = Instant::now();
            }
        }
        if !hooked.is_null() {
            sync_frame(hooked);
            if hooked_at.elapsed() < START_SIZE_WINDOW && !USER_RESIZED.load(Ordering::SeqCst) {
                apply_start_size(hooked);
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

unsafe extern "system" fn find_callback(hwnd: Hwnd, out: isize) -> i32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid != unsafe { GetCurrentProcessId() } || unsafe { IsWindowVisible(hwnd) } == 0 {
        return 1;
    }
    let mut buf = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) }.max(0) as usize;
    // Class (and title) is "ELDEN RING™"; the process also owns hidden
    // "IME"/"DIEmWin" helper windows.
    if String::from_utf16_lossy(&buf[..len]).starts_with("ELDEN RING") {
        unsafe { *(out as *mut Hwnd) = hwnd };
        return 0;
    }
    1
}

fn find_game_window() -> Option<Hwnd> {
    let mut found: Hwnd = null_mut();
    unsafe { EnumWindows(find_callback, &mut found as *mut Hwnd as isize) };
    (!found.is_null()).then_some(found)
}

fn hook(hwnd: Hwnd) {
    // Store the current procedure before swapping, so a message arriving
    // between the swap and the store below never sees 0.
    ORIG_WNDPROC.store(unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) }, Ordering::SeqCst);
    let prev = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wnd_proc as *const () as isize) };
    if prev == 0 {
        logger::error("Couldn't subclass the game window - resizing unavailable.");
        return;
    }
    ORIG_WNDPROC.store(prev, Ordering::SeqCst);
    FRAME_ADDED.store(false, Ordering::SeqCst);
    logger::log(&format!("Game window found and hooked (DPI {}).", unsafe { GetDpiForWindow(hwnd) }));
}

/// Adds or removes `WS_THICKFRAME` to match `Resizable`, windowed mode only
/// (fullscreen/borderless windows have no caption and are left alone).
fn sync_frame(hwnd: Hwnd) {
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    let windowed = style & WS_CAPTION == WS_CAPTION;
    let has_frame = style & WS_THICKFRAME != 0;
    let want = windowed && config::get_bool("Resizable", true);

    if !windowed {
        FRAME_ADDED.store(false, Ordering::SeqCst);
        return;
    }
    let new_style = if want && !has_frame {
        FRAME_ADDED.store(true, Ordering::SeqCst);
        logger::log("Resize border added.");
        style | WS_THICKFRAME
    } else if !want && has_frame && FRAME_ADDED.load(Ordering::SeqCst) {
        FRAME_ADDED.store(false, Ordering::SeqCst);
        logger::log("Resize border removed (Resizable=false).");
        style & !WS_THICKFRAME
    } else {
        if want {
            // Border already there (e.g. the window was re-hooked) - make sure
            // the message handling is on for it.
            FRAME_ADDED.store(true, Ordering::SeqCst);
        }
        return;
    };
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, new_style as isize);
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Outer window size minus client size for the window's current style and
/// DPI - computed rather than read from the live rects, which are 0x0 while
/// the window is minimized.
fn frame_size(hwnd: Hwnd) -> (i32, i32) {
    let mut rect = Rect::default();
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        AdjustWindowRectExForDpi(&mut rect, style, 0, ex_style, GetDpiForWindow(hwnd));
    }
    (rect.right - rect.left, rect.bottom - rect.top)
}

/// Client size from `Width`/`Height`; `None` if both are empty. With only
/// one set, the other follows `AspectWidth`:`AspectHeight` (or keeps the
/// current size if the aspect lock is off).
fn start_client_size(current: (i32, i32)) -> Option<(i32, i32)> {
    let w = config::get_int("Width", 0);
    let h = config::get_int("Height", 0);
    let aspect_w = config::get_int("AspectWidth", 16) as i64;
    let aspect_h = config::get_int("AspectHeight", 9) as i64;
    let aspect = aspect_w > 0 && aspect_h > 0;
    let size = match (w > 0, h > 0) {
        (false, false) => return None,
        (true, true) => (w, h),
        (true, false) if aspect => (w, ((w as i64 * aspect_h + aspect_w / 2) / aspect_w) as i32),
        (false, true) if aspect => (((h as i64 * aspect_w + aspect_h / 2) / aspect_h) as i32, h),
        (true, false) => (w, current.1),
        (false, true) => (current.0, h),
    };
    let (min_w, min_h) = min_client();
    Some((size.0.max(min_w), size.1.max(min_h)))
}

/// Resizes the window so its client area is the ini's startup size, keeping
/// its position. No-op when already that size, minimized, or not windowed.
fn apply_start_size(hwnd: Hwnd) {
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    if style & WS_CAPTION != WS_CAPTION || unsafe { IsIconic(hwnd) } != 0 {
        return;
    }
    let mut client = Rect::default();
    let mut window = Rect::default();
    unsafe {
        GetClientRect(hwnd, &mut client);
        GetWindowRect(hwnd, &mut window);
    }
    let current = (client.right - client.left, client.bottom - client.top);
    let Some((w, h)) = start_client_size(current) else {
        return;
    };
    if current == (w, h) {
        return;
    }
    // Frame measured from the live rects (same DPI context as the resize
    // call below) rather than `frame_size`, which uses the window's DPI.
    let frame_w = (window.right - window.left) - current.0;
    let frame_h = (window.bottom - window.top) - current.1;
    unsafe {
        SetWindowPos(hwnd, null_mut(), 0, 0, w + frame_w, h + frame_h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
    logger::log(&format!("Window set to {w}x{h} (was {}x{}).", current.0, current.1));
}

fn min_client() -> (i32, i32) {
    (config::get_int("MinWidth", 160).max(1), config::get_int("MinHeight", 90).max(1))
}

/// Resizes the dragged `rect` (outer window, screen coords) so its client
/// area respects the minimum and, if set, the aspect ratio - moving only
/// the edges the user is actually dragging.
fn constrain(hwnd: Hwnd, edge: usize, rect: &mut Rect) {
    let (frame_w, frame_h) = frame_size(hwnd);
    let (min_w, min_h) = min_client();
    let mut w = (rect.right - rect.left - frame_w).max(min_w);
    let mut h = (rect.bottom - rect.top - frame_h).max(min_h);

    let aspect_w = config::get_int("AspectWidth", 16);
    let aspect_h = config::get_int("AspectHeight", 9);
    if aspect_w > 0 && aspect_h > 0 {
        let (aw, ah) = (aspect_w as i64, aspect_h as i64);
        let height_for = |w: i32| ((w as i64 * ah + aw / 2) / aw) as i32;
        let width_for = |h: i32| ((h as i64 * aw + ah / 2) / ah) as i32;
        // Top/bottom edges drive the width; every other edge and corner
        // drives the height from the width.
        if matches!(edge, WMSZ_TOP | WMSZ_BOTTOM) {
            w = width_for(h);
        } else {
            h = height_for(w);
        }
        if h < min_h {
            h = min_h;
            w = width_for(h);
        }
        if w < min_w {
            w = min_w;
            h = height_for(w);
        }
    }

    let outer_w = w + frame_w;
    let outer_h = h + frame_h;
    if matches!(edge, WMSZ_LEFT | WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT) {
        rect.left = rect.right - outer_w;
    } else {
        rect.right = rect.left + outer_w;
    }
    if matches!(edge, WMSZ_TOP | WMSZ_TOPLEFT | WMSZ_TOPRIGHT) {
        rect.top = rect.bottom - outer_h;
    } else {
        rect.bottom = rect.top + outer_h;
    }
}

unsafe extern "system" fn wnd_proc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> isize {
    let orig = ORIG_WNDPROC.load(Ordering::SeqCst);
    if FRAME_ADDED.load(Ordering::SeqCst) {
        match msg {
            WM_ENTERSIZEMOVE => USER_RESIZED.store(true, Ordering::SeqCst),
            WM_SETCURSOR => {
                let hit = (lparam & 0xFFFF) as u32;
                if (HT_SIZE_FIRST..=HT_SIZE_LAST).contains(&hit) {
                    return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
                }
            }
            WM_GETMINMAXINFO => {
                let result = unsafe { CallWindowProcW(orig, hwnd, msg, wparam, lparam) };
                let (frame_w, frame_h) = frame_size(hwnd);
                let (min_w, min_h) = min_client();
                let info = unsafe { &mut *(lparam as *mut MinMaxInfo) };
                info.min_track_size = Point { x: min_w + frame_w, y: min_h + frame_h };
                return result;
            }
            WM_SIZING => {
                unsafe { CallWindowProcW(orig, hwnd, msg, wparam, lparam) };
                constrain(hwnd, wparam, unsafe { &mut *(lparam as *mut Rect) });
                return 1;
            }
            _ => {}
        }
    }
    unsafe { CallWindowProcW(orig, hwnd, msg, wparam, lparam) }
}
