//! Look and input plumbing for the shared config menu - copied from
//! SoulsTeleport's `ui.rs` (2026-10-02): embedded Noto Sans + optional CJK,
//! resolution scale, theme, mouse fed to ImGui by hand, and the cursor
//! unpin hooks. See SoulsTeleport's README for why each piece exists (the
//! game hides/confines the cursor and reads the mouse through raw input).
//! SoulsTeleport keeps its own copy until a release of it moves over.

use std::sync::atomic::{AtomicUsize, Ordering};

use hudhook::imgui::{self, Context, MouseButton};
use hudhook::mh::{MH_ApplyQueued, MhHook};

use super::MENU_OPEN;
use crate::logger;

// Latin + Latin-1 + Latin Extended-A/B + Vietnamese (Latin Extended
// Additional) + general punctuation - enough for Vietnamese / Western
// character names. The default ImGui font has only ASCII.
pub(super) static GLYPH_RANGES: [u32; 9] = [0x0020, 0x024F, 0x0300, 0x036F, 0x1E00, 0x1EFF, 0x2000, 0x206F, 0];

/// Main font, always available: Noto Sans (SIL OFL 1.1, see
/// `assets/NotoSans-OFL.txt`) embedded in the DLL, subset to the ranges in
/// [GLYPH_RANGES] (~90 KB) - no dependency on the user's installed fonts, a
/// Windows install on another drive, or Proton/Steam Deck (whose
/// `C:\Windows\Fonts` usually has none of the Windows fonts).
pub(super) static EMBEDDED_FONT: &[u8] = include_bytes!("../../../assets/NotoSans-Subset.ttf");

/// Optional CJK coverage for character names, merged in from the first one
/// of these found in the real Windows fonts folder (`GetWindowsDirectoryW`,
/// not a hard-coded `C:`). None found = CJK names show as `?`, nothing else
/// breaks. Korean is not covered: ImGui's Hangul range is the full 11k
/// syllables - too big for the atlas at this raster size.
const CJK_FONT_FILES: [&str; 5] = ["msyh.ttc", "msjh.ttc", "meiryo.ttc", "msgothic.ttc", "simsun.ttc"];
/// Font is rasterized once at this size (big enough to stay sharp when
/// scaled up on 4K) and shown at `FONT_AT_1080P * scale` via
/// `font_global_scale`.
pub(super) const FONT_RASTER_BASE: f32 = 40.0;

/// Menu is laid out for a 1080p game window; everything scales with the
/// game window's height from there.
const REFERENCE_HEIGHT: f32 = 1080.0;
pub(super) const FONT_AT_1080P: f32 = 22.0;
const MIN_SCALE: f32 = 0.6;
const MAX_SCALE: f32 = 3.0;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetWindowsDirectoryW(buffer: *mut u16, size: u32) -> u32;
    fn GetCurrentProcessId() -> u32;
}

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetForegroundWindow() -> *mut std::ffi::c_void;
    fn GetWindowThreadProcessId(hwnd: *mut std::ffi::c_void, pid: *mut u32) -> u32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn ScreenToClient(hwnd: *mut std::ffi::c_void, point: *mut Point) -> i32;
    fn GetClientRect(hwnd: *mut std::ffi::c_void, rect: *mut Rect) -> i32;
    fn ClipCursor(rect: *const Rect) -> i32;
    fn GetAsyncKeyState(vk: i32) -> i16;
}

const VK_LBUTTON: i32 = 0x01;
const VK_RBUTTON: i32 = 0x02;

/// The game window, if it's the foreground window (ours, not another app).
fn game_window() -> Option<*mut std::ffi::c_void> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    (pid == unsafe { GetCurrentProcessId() }).then_some(hwnd)
}

/// Feeds ImGui the mouse ourselves while the menu is open. First in-game
/// test (2026-09-25): no usable cursor - the game hides and confines the
/// cursor and reads the mouse through raw input, so hudhook's `WM_MOUSEMOVE`
/// path gets nothing useful. Polled from the OS each frame instead, and the
/// cursor is released (`ClipCursor(NULL)`) so it can reach the menu.
/// The position is mapped from window-client pixels to the swapchain size
/// ImGui draws in (hudhook's `display_size`), which also covers a Windows
/// display scale (the game isn't DPI-aware, so the two can differ).
pub(super) fn feed_mouse(ctx: &mut Context, buttons_down: &mut [bool; 2]) {
    let Some(hwnd) = game_window() else {
        return;
    };
    unsafe { ClipCursor(std::ptr::null()) };

    let mut pt = Point::default();
    let mut rect = Rect::default();
    let ok = unsafe { GetCursorPos(&mut pt) != 0 && ScreenToClient(hwnd, &mut pt) != 0 && GetClientRect(hwnd, &mut rect) != 0 };
    let client_w = (rect.right - rect.left) as f32;
    let client_h = (rect.bottom - rect.top) as f32;
    let io = ctx.io_mut();
    if ok && client_w > 0.0 && client_h > 0.0 {
        let [dw, dh] = io.display_size;
        io.add_mouse_pos_event([pt.x as f32 * dw / client_w, pt.y as f32 * dh / client_h]);
    }
    for (i, (vk, button)) in [(VK_LBUTTON, MouseButton::Left), (VK_RBUTTON, MouseButton::Right)].into_iter().enumerate() {
        let down = unsafe { GetAsyncKeyState(vk) } < 0;
        if down != buttons_down[i] {
            buttons_down[i] = down;
            io.add_mouse_button_event(button, down);
        }
    }
}

fn windows_fonts_dir() -> Option<std::path::PathBuf> {
    let mut buf = [0u16; 260];
    let len = unsafe { GetWindowsDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if len == 0 || len >= buf.len() {
        return None;
    }
    Some(std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len])).join("Fonts"))
}

pub(super) fn find_cjk_font() -> Option<(&'static str, Vec<u8>)> {
    let dir = windows_fonts_dir()?;
    CJK_FONT_FILES.iter().find_map(|name| std::fs::read(dir.join(name)).ok().map(|b| (*name, b)))
}

pub(super) fn scale_for(display_size: [f32; 2]) -> f32 {
    if display_size[1] <= 0.0 {
        return 1.0;
    }
    (display_size[1] / REFERENCE_HEIGHT).clamp(MIN_SCALE, MAX_SCALE)
}

// --- Theme -----------------------------------------------------------------
// Warm near-black, translucent panel with muted gold accents - in the spirit
// of the game's own menus, instead of ImGui's default blue.

pub(super) const GOLD: [f32; 4] = [0.84, 0.72, 0.47, 1.00];
pub(super) const GOLD_DIM: [f32; 4] = [0.62, 0.54, 0.38, 1.00];
const TEXT: [f32; 4] = [0.93, 0.90, 0.83, 1.00];
pub(super) const TEXT_MUTED: [f32; 4] = [0.60, 0.57, 0.52, 1.00];
pub(super) const ERROR_TEXT: [f32; 4] = [0.90, 0.48, 0.40, 1.00];

fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

pub(super) fn apply_theme(style: &mut imgui::Style) {
    use imgui::StyleColor as C;

    style.window_padding = [18.0, 16.0];
    style.window_rounding = 10.0;
    style.window_border_size = 1.0;
    style.window_title_align = [0.5, 0.5];
    style.frame_padding = [14.0, 9.0];
    style.frame_rounding = 6.0;
    style.frame_border_size = 1.0;
    style.item_spacing = [10.0, 9.0];
    style.popup_rounding = 6.0;
    style.popup_border_size = 1.0;
    style.scrollbar_size = 12.0;
    style.scrollbar_rounding = 6.0;
    style.grab_rounding = 6.0;

    style[C::Text] = TEXT;
    style[C::TextDisabled] = TEXT_MUTED;
    style[C::WindowBg] = rgba(16, 14, 12, 0.93);
    style[C::PopupBg] = rgba(22, 19, 16, 0.97);
    style[C::Border] = rgba(158, 132, 86, 0.55);
    style[C::BorderShadow] = [0.0, 0.0, 0.0, 0.0];

    style[C::TitleBg] = rgba(28, 24, 19, 0.96);
    style[C::TitleBgActive] = rgba(40, 33, 24, 0.98);
    style[C::TitleBgCollapsed] = rgba(28, 24, 19, 0.80);

    style[C::FrameBg] = rgba(34, 29, 23, 0.90);
    style[C::FrameBgHovered] = rgba(58, 48, 34, 0.90);
    style[C::FrameBgActive] = rgba(72, 59, 40, 0.95);

    style[C::Button] = rgba(38, 32, 25, 0.95);
    style[C::ButtonHovered] = rgba(86, 69, 42, 0.95);
    style[C::ButtonActive] = rgba(120, 95, 55, 1.00);

    style[C::Header] = rgba(60, 50, 35, 0.80);
    style[C::HeaderHovered] = rgba(86, 69, 42, 0.90);
    style[C::HeaderActive] = rgba(120, 95, 55, 1.00);

    style[C::Separator] = rgba(158, 132, 86, 0.40);
    style[C::SeparatorHovered] = rgba(200, 168, 110, 0.70);
    style[C::SeparatorActive] = rgba(214, 184, 120, 1.00);

    style[C::ScrollbarBg] = rgba(16, 14, 12, 0.40);
    style[C::ScrollbarGrab] = rgba(90, 76, 54, 0.80);
    style[C::ScrollbarGrabHovered] = rgba(120, 100, 68, 0.90);
    style[C::ScrollbarGrabActive] = rgba(158, 132, 86, 1.00);

    style[C::ResizeGrip] = rgba(158, 132, 86, 0.25);
    style[C::ResizeGripHovered] = rgba(200, 168, 110, 0.60);
    style[C::ResizeGripActive] = rgba(214, 184, 120, 0.90);

    style[C::CheckMark] = GOLD;
    style[C::SliderGrab] = GOLD_DIM;
    style[C::SliderGrabActive] = GOLD;
    style[C::NavHighlight] = GOLD;
}

// --- Cursor unpin -------------------------------------------------------------
// The game keeps re-confining the cursor to its window (`ClipCursor`) and
// recentering it (`SetCursorPos`) for mouse-look, so releasing it once per
// frame isn't enough. Same approach as QuestPath ("cursor-unpin" in its
// log): hook both and, while the menu is open, don't let the game move or
// confine the cursor.

type SetCursorPosFn = unsafe extern "system" fn(x: i32, y: i32) -> i32;
type ClipCursorFn = unsafe extern "system" fn(rect: *const Rect) -> i32;

static SET_CURSOR_POS_ORIG: AtomicUsize = AtomicUsize::new(0);
static CLIP_CURSOR_ORIG: AtomicUsize = AtomicUsize::new(0);

unsafe extern "system" fn set_cursor_pos_hook(x: i32, y: i32) -> i32 {
    if MENU_OPEN.load(Ordering::Relaxed) {
        return 1; // pretend it worked, don't move the cursor
    }
    let orig = SET_CURSOR_POS_ORIG.load(Ordering::Relaxed);
    unsafe { std::mem::transmute::<usize, SetCursorPosFn>(orig)(x, y) }
}

unsafe extern "system" fn clip_cursor_hook(rect: *const Rect) -> i32 {
    let rect = if MENU_OPEN.load(Ordering::Relaxed) { std::ptr::null() } else { rect };
    let orig = CLIP_CURSOR_ORIG.load(Ordering::Relaxed);
    unsafe { std::mem::transmute::<usize, ClipCursorFn>(orig)(rect) }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleA(name: *const u8) -> *mut std::ffi::c_void;
    fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
}

fn hook_user32(name: &str, detour: *mut std::ffi::c_void, orig_slot: &AtomicUsize) -> bool {
    let user32 = unsafe { GetModuleHandleA(c"user32.dll".as_ptr() as *const u8) };
    let c_name = format!("{name}\0");
    let target = unsafe { GetProcAddress(user32, c_name.as_ptr()) };
    if user32.is_null() || target.is_null() {
        return false;
    }
    // MinHook is already initialized by `Hudhook::builder()`.
    match unsafe { MhHook::new(target, detour) } {
        Ok(hook) => {
            orig_slot.store(hook.trampoline() as usize, Ordering::Relaxed);
            unsafe { hook.queue_enable() }.is_ok()
        }
        Err(_) => false,
    }
}

pub(super) fn install_cursor_hooks() {
    let set_pos = hook_user32("SetCursorPos", set_cursor_pos_hook as *mut std::ffi::c_void, &SET_CURSOR_POS_ORIG);
    let clip = hook_user32("ClipCursor", clip_cursor_hook as *mut std::ffi::c_void, &CLIP_CURSOR_ORIG);
    let applied = unsafe { MH_ApplyQueued() }.ok().is_ok();
    if set_pos && clip && applied {
        logger::log("Menu: cursor unpin hooks applied.");
    } else {
        logger::error(&format!(
            "Menu: cursor unpin hooks incomplete (SetCursorPos {set_pos}, ClipCursor {clip}, apply {applied}) - the cursor may stay locked while the menu is open."
        ));
    }
}
