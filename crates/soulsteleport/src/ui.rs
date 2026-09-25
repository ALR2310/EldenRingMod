//! In-game menu (Dear ImGui drawn over the game's own DX12 frames via
//! hudhook - the same stack SoulsChat uses) listing co-op partners; clicking
//! one teleports there.
//!
//! Threading: [MenuRenderLoop] runs on the game's render thread (inside the
//! hooked `Present`); everything touching the session, Steam and the warp
//! stays on the game task in `warp.rs`. The two only share [SHARED] (a
//! snapshot the task refreshes while the menu is open, plus status text) and
//! [REQUEST] (the partner the player clicked, picked up by the task).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::imgui::{self, Condition, MouseButton, Context, FontConfig, FontGlyphRanges, FontSource, Io, Ui};
use hudhook::mh::{MH_ApplyQueued, MhHook};
use hudhook::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use hudhook::{BeforeWndProc, Hudhook, ImguiRenderLoop, MessageFilter, RenderContext};

use common::{config, logger};

pub static MENU_OPEN: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
pub struct PartnerRow {
    pub steam_id: u64,
    pub character_name: String,
    pub steam_name: String,
    pub role: &'static str,
}

#[derive(Default)]
pub struct Shared {
    pub in_session: bool,
    pub partners: Vec<PartnerRow>,
    /// Last outcome/progress line ("Locating X...", "X did not respond").
    /// Private: set through [set_status] / [Shared::set_status] so
    /// `status_set` stays in sync.
    status: String,
    /// When `status` was last set - it's cleared [STATUS_TTL] later (unless a
    /// request is still in flight), so no line can hang around forever.
    status_set: Option<std::time::Instant>,
    /// A WHERE is in flight - buttons disabled until it resolves.
    pub busy: bool,
}

pub static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

/// SteamID of the partner the player clicked, for the game task to take.
pub static REQUEST: Mutex<Option<u64>> = Mutex::new(None);

pub fn with_shared<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    let mut guard = SHARED.lock().unwrap();
    f(guard.get_or_insert_with(Shared::default))
}

/// How long a status line stays before it's cleared. User report
/// (2026-09-25): the line could get stuck (e.g. "Teleporting to X..." was
/// never cleared after a successful warp).
const STATUS_TTL: std::time::Duration = std::time::Duration::from_secs(15);

impl Shared {
    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = text.into();
        self.status_set = Some(std::time::Instant::now());
    }

    /// Current status line, clearing it first if it expired.
    fn current_status(&mut self) -> String {
        if !self.busy && self.status_set.is_some_and(|t| t.elapsed() >= STATUS_TTL) {
            self.status.clear();
            self.status_set = None;
        }
        self.status.clone()
    }
}

pub fn set_status(text: impl Into<String>) {
    with_shared(|s| s.set_status(text));
}

/// Status text is kept across close/reopen on purpose (user request,
/// 2026-09-25): reopening mid-request should still say what's going on.
pub fn toggle() {
    MENU_OPEN.fetch_xor(true, Ordering::Relaxed);
}

// Latin + Latin-1 + Latin Extended-A/B + Vietnamese (Latin Extended
// Additional) + general punctuation - enough for Vietnamese / Western
// character names. The default ImGui font has only ASCII.
static GLYPH_RANGES: [u32; 9] = [0x0020, 0x024F, 0x0300, 0x036F, 0x1E00, 0x1EFF, 0x2000, 0x206F, 0];

/// Main font, always available: Noto Sans (SIL OFL 1.1, see
/// `assets/NotoSans-OFL.txt`) embedded in the DLL, subset to the ranges in
/// [GLYPH_RANGES] (~90 KB) - no dependency on the user's installed fonts, a
/// Windows install on another drive, or Proton/Steam Deck (whose
/// `C:\Windows\Fonts` usually has none of the Windows fonts).
static EMBEDDED_FONT: &[u8] = include_bytes!("../assets/NotoSans-Subset.ttf");

/// Optional CJK coverage for character names, merged in from the first one
/// of these found in the real Windows fonts folder (`GetWindowsDirectoryW`,
/// not a hard-coded `C:`). None found = CJK names show as `?`, nothing else
/// breaks. Korean is not covered: ImGui's Hangul range is the full 11k
/// syllables - too big for the atlas at this raster size.
const CJK_FONT_FILES: [&str; 5] = ["msyh.ttc", "msjh.ttc", "meiryo.ttc", "msgothic.ttc", "simsun.ttc"];
/// Font is rasterized once at this size (big enough to stay sharp when
/// scaled up on 4K) and shown at `FONT_AT_1080P * scale` via
/// `font_global_scale`.
const FONT_RASTER_BASE: f32 = 40.0;
const FONT_RASTER_MAX: f32 = 64.0;

/// `MenuScale` ini key: extra multiplier on top of the automatic resolution
/// scale (1.0 = as designed). Applies to text, widgets and the menu's size,
/// not its position (see [MenuRenderLoop::render]). Read once at startup.
const KEY_MENU_SCALE: &str = "MenuScale";
const MENU_SCALE_MIN: f32 = 0.5;
const MENU_SCALE_MAX: f32 = 3.0;

fn menu_scale() -> f32 {
    let v = config::get_double(KEY_MENU_SCALE, 1.0) as f32;
    if v.is_finite() { v.clamp(MENU_SCALE_MIN, MENU_SCALE_MAX) } else { 1.0 }
}

/// Raster size for the font atlas: bigger when `MenuScale` makes the text
/// bigger, so it stays sharp (capped - the atlas also holds CJK glyphs).
fn font_raster_size() -> f32 {
    (FONT_RASTER_BASE * menu_scale()).clamp(FONT_RASTER_BASE, FONT_RASTER_MAX)
}
/// Menu is laid out for a 1080p game window; everything scales with the
/// game window's height from there.
const REFERENCE_HEIGHT: f32 = 1080.0;
const FONT_AT_1080P: f32 = 22.0;
const MIN_SCALE: f32 = 0.6;
const MAX_SCALE: f32 = 3.0;
const WINDOW_SIZE_AT_1080P: [f32; 2] = [420.0, 360.0];
const WINDOW_POS_AT_1080P: [f32; 2] = [60.0, 60.0];

/// Where the menu's position/size is remembered: `[Menu]` in the ini, in
/// 1080p units (divided by the UI scale) so a saved layout still fits after
/// changing resolution. -1 = default layout.
const KEY_X: &str = "MenuX";
const KEY_Y: &str = "MenuY";
const KEY_W: &str = "MenuWidth";
const KEY_H: &str = "MenuHeight";

static INI_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Tells the menu which ini to save its layout into. Call before [install].
pub fn set_ini_path(path: String) {
    let _ = INI_PATH.set(path);
}

/// Saved layout (1080p units), falling back to the default for any value
/// that's missing, -1, or not a sane size.
fn saved_layout() -> ([f32; 2], [f32; 2]) {
    let read = |key: &str, default: f32, min: f32| {
        let v = config::get_double(key, -1.0) as f32;
        if v.is_finite() && v >= min { v } else { default }
    };
    (
        [read(KEY_X, WINDOW_POS_AT_1080P[0], 0.0), read(KEY_Y, WINDOW_POS_AT_1080P[1], 0.0)],
        [read(KEY_W, WINDOW_SIZE_AT_1080P[0], 120.0), read(KEY_H, WINDOW_SIZE_AT_1080P[1], 80.0)],
    )
}

fn save_layout(pos: [f32; 2], size: [f32; 2]) {
    let Some(path) = INI_PATH.get() else {
        return;
    };
    let fmt = |v: f32| format!("{:.0}", v.max(0.0));
    let ok = config::set_values(
        path,
        &[(KEY_X, fmt(pos[0])), (KEY_Y, fmt(pos[1])), (KEY_W, fmt(size[0])), (KEY_H, fmt(size[1]))],
    );
    if !ok {
        logger::error("Menu: couldn't save the menu position to the ini.");
    }
}

#[derive(Default)]
struct MenuRenderLoop {
    /// Unscaled ImGui style, to re-derive from on every scale change
    /// (`scale_all_sizes` compounds if applied to an already-scaled style).
    base_style: Option<imgui::Style>,
    /// Scale currently applied (0 = none yet): resolution x `MenuScale`.
    scale: f32,
    /// Resolution-only part of it, for the menu's position.
    pos_scale: f32,
    /// `MenuScale` from the ini, read once at startup.
    menu_scale: f32,
    /// Set when the scale changes, so the next frame re-applies the window's
    /// size/position once (the player can still move/resize it after).
    relayout: bool,
    /// Left/right mouse button state last fed to ImGui (see [feed_mouse]).
    buttons_down: [bool; 2],
    /// Menu position/size (1080p units) as last seen / saved - saved to the
    /// ini when it changed and the player let go of the mouse.
    layout_seen: Option<([f32; 2], [f32; 2])>,
    layout_saved: Option<([f32; 2], [f32; 2])>,
}

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
fn feed_mouse(ctx: &mut Context, buttons_down: &mut [bool; 2]) {
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

fn find_cjk_font() -> Option<(&'static str, Vec<u8>)> {
    let dir = windows_fonts_dir()?;
    CJK_FONT_FILES.iter().find_map(|name| std::fs::read(dir.join(name)).ok().map(|b| (*name, b)))
}

fn scale_for(display_size: [f32; 2]) -> f32 {
    if display_size[1] <= 0.0 {
        return 1.0;
    }
    (display_size[1] / REFERENCE_HEIGHT).clamp(MIN_SCALE, MAX_SCALE)
}

impl ImguiRenderLoop for MenuRenderLoop {
    fn initialize<'a>(&'a mut self, ctx: &mut Context, _render_context: &'a mut dyn RenderContext) {
        // No imgui.ini next to the game exe.
        ctx.set_ini_filename(None);

        let main = FontSource::TtfData {
            data: EMBEDDED_FONT,
            size_pixels: font_raster_size(),
            config: Some(FontConfig {
                glyph_ranges: FontGlyphRanges::from_slice(&GLYPH_RANGES),
                ..FontConfig::default()
            }),
        };
        // `add_font` copies the data into the atlas, so a local Vec is fine.
        let cjk = find_cjk_font();
        match &cjk {
            Some((name, bytes)) => {
                ctx.fonts().add_font(&[
                    main,
                    FontSource::TtfData {
                        data: bytes,
                        size_pixels: font_raster_size(),
                        config: Some(FontConfig {
                            glyph_ranges: FontGlyphRanges::chinese_simplified_common(),
                            ..FontConfig::default()
                        }),
                    },
                ]);
                logger::log(&format!("Menu: embedded Noto Sans + CJK from {name}."));
            }
            None => {
                ctx.fonts().add_font(&[main]);
                logger::log("Menu: embedded Noto Sans (no system CJK font found).");
            }
        }
        self.menu_scale = menu_scale();
        if self.menu_scale != 1.0 {
            logger::log(&format!("Menu: MenuScale {:.2}.", self.menu_scale));
        }
        apply_theme(ctx.style_mut());
        self.base_style = Some(*ctx.style());
        logger::log("Menu: ImGui initialized.");
    }

    fn before_render<'a>(&'a mut self, ctx: &mut Context, _render_context: &'a mut dyn RenderContext) {
        // The game hides the OS cursor - ImGui draws its own, only while open.
        let open = MENU_OPEN.load(Ordering::Relaxed);
        ctx.io_mut().mouse_draw_cursor = open;
        if open {
            feed_mouse(ctx, &mut self.buttons_down);
        } else if self.buttons_down != [false; 2] {
            // Don't leave a button "held" in ImGui across a close/reopen.
            self.buttons_down = [false; 2];
            ctx.io_mut().add_mouse_button_event(MouseButton::Left, false);
            ctx.io_mut().add_mouse_button_event(MouseButton::Right, false);
        }

        // Follow the game window's resolution (and changes to it).
        // Resolution scale x the player's own MenuScale.
        self.pos_scale = scale_for(ctx.io().display_size);
        let menu_scale = if self.menu_scale > 0.0 { self.menu_scale } else { 1.0 };
        let scale = self.pos_scale * menu_scale;
        if (scale - self.scale).abs() > 0.01 {
            if let Some(base) = self.base_style {
                let mut style = base;
                style.scale_all_sizes(scale);
                // `ScaleAllSizes` floors the cursor scale to a whole number
                // (`ImFloor`, imgui.cpp) - any scale below 1 made it 0, so the
                // cursor vanished in windows under 1080 px tall (user report:
                // 1680x945 and smaller; clicks still landed, it just wasn't
                // drawn). Set it unrounded instead.
                style.mouse_cursor_scale = base.mouse_cursor_scale * scale;
                *ctx.style_mut() = style;
            }
            ctx.io_mut().font_global_scale = FONT_AT_1080P * scale / font_raster_size();
            if self.scale != 0.0 {
                logger::log(&format!("Menu: UI scale {:.2} -> {scale:.2}.", self.scale));
            }
            self.scale = scale;
            self.relayout = true;
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        if !MENU_OPEN.load(Ordering::Relaxed) {
            return;
        }

        // Size follows the full scale (bigger text needs a bigger window);
        // position only the resolution, so changing MenuScale doesn't move
        // the menu away from where the player put it.
        let scale = if self.scale > 0.0 { self.scale } else { 1.0 };
        let pos_scale = if self.pos_scale > 0.0 { self.pos_scale } else { 1.0 };
        let cond = if std::mem::take(&mut self.relayout) { Condition::Always } else { Condition::FirstUseEver };
        let (pos, size) = saved_layout();
        let mut keep_open = true;
        let mut reset_layout = false;
        let mut layout = None;
        ui.window("##souls_teleport")
            .title_bar(false)
            .size(size.map(|v| v * scale), cond)
            .position(pos.map(|v| v * pos_scale), cond)
            .collapsible(false)
            .build(|| {
                layout = Some((ui.window_pos().map(|v| v / pos_scale), ui.window_size().map(|v| v / scale)));
                draw_contents(ui, &mut keep_open, &mut reset_layout);
            });

        if reset_layout {
            // Back to the default layout: -1 in the ini, and re-apply it on
            // the next frame (Condition::Always). The saved/seen baseline is
            // dropped so that default layout isn't then saved back as numbers.
            if let Some(path) = INI_PATH.get() {
                let reset = |k| (k, "-1".to_string());
                config::set_values(path, &[reset(KEY_X), reset(KEY_Y), reset(KEY_W), reset(KEY_H)]);
            }
            self.relayout = true;
            self.layout_saved = None;
            self.layout_seen = None;
            layout = None;
        }

        // Remember where the player dragged/resized the menu - saved once
        // they let go of the mouse (not every frame of a drag).
        if let Some(now) = layout {
            if self.layout_saved.is_none() {
                self.layout_saved = Some(now);
            }
            self.layout_seen = Some(now);
        }
        let mouse_held = self.buttons_down[0];
        if !mouse_held {
            if let Some(seen) = self.layout_seen {
                let changed = self.layout_saved.is_none_or(|(p, s)| {
                    (p[0] - seen.0[0]).abs() >= 1.0
                        || (p[1] - seen.0[1]).abs() >= 1.0
                        || (s[0] - seen.1[0]).abs() >= 1.0
                        || (s[1] - seen.1[1]).abs() >= 1.0
                });
                if changed {
                    save_layout(seen.0, seen.1);
                    self.layout_saved = Some(seen);
                }
            }
        }

        if !keep_open {
            MENU_OPEN.store(false, Ordering::Relaxed);
        }
    }

    fn before_wnd_proc(&self, _hwnd: HWND, umsg: u32, _wparam: WPARAM, _lparam: LPARAM) -> BeforeWndProc {
        // Second in-game test: still no cursor. hudhook turns the game's
        // relative raw-input mouse deltas into `mouse_pos + delta` - starting
        // from ImGui's "no position" value (-FLT_MAX), so the position never
        // becomes valid (no cursor drawn), and those events also overrode the
        // absolute position `feed_mouse` sets. While the menu is open, the
        // mouse comes only from `feed_mouse`; keyboard messages still reach
        // ImGui as usual.
        const WM_INPUT: u32 = 0x00FF;
        const WM_MOUSEFIRST: u32 = 0x0200;
        const WM_MOUSELAST: u32 = 0x020E;
        let mouse_msg = umsg == WM_INPUT || (WM_MOUSEFIRST..=WM_MOUSELAST).contains(&umsg);
        if mouse_msg && MENU_OPEN.load(Ordering::Relaxed) { BeforeWndProc::Break } else { BeforeWndProc::Continue }
    }

    fn message_filter(&self, _io: &Io) -> MessageFilter {
        // While open, keyboard/mouse window messages go to the menu only. The
        // game itself reads input through DirectInput, not these messages
        // (see `input_block` for what actually stops the camera).
        if MENU_OPEN.load(Ordering::Relaxed) { MessageFilter::InputAll } else { MessageFilter::empty() }
    }
}

// --- Theme -----------------------------------------------------------------
// Warm near-black, translucent panel with muted gold accents - in the spirit
// of the game's own menus, instead of ImGui's default blue.

const GOLD: [f32; 4] = [0.84, 0.72, 0.47, 1.00];
const GOLD_DIM: [f32; 4] = [0.62, 0.54, 0.38, 1.00];
const TEXT: [f32; 4] = [0.93, 0.90, 0.83, 1.00];
const TEXT_MUTED: [f32; 4] = [0.60, 0.57, 0.52, 1.00];
const ERROR_TEXT: [f32; 4] = [0.90, 0.48, 0.40, 1.00];

fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

fn apply_theme(style: &mut imgui::Style) {
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

fn role_color(role: &str) -> [f32; 4] {
    match role {
        "Host" => GOLD,
        "Invader" | "Bloody Finger" | "Recusant" => ERROR_TEXT,
        "Unknown" | "Other" => TEXT_MUTED,
        _ => GOLD_DIM,
    }
}

fn centered_text(ui: &Ui, color: [f32; 4], text: &str) {
    let avail = ui.content_region_avail()[0];
    let width = ui.calc_text_size(text)[0];
    let x = ui.cursor_pos()[0] + ((avail - width) * 0.5).max(0.0);
    ui.set_cursor_pos([x, ui.cursor_pos()[1]]);
    ui.text_colored(color, text);
}

fn draw_contents(ui: &Ui, keep_open: &mut bool, reset_layout: &mut bool) {
    let (in_session, partners, status, busy) =
        with_shared(|s| (s.in_session, s.partners.clone(), s.current_status(), s.busy));

    // Header: small "reset layout" button on the left, mod name centered,
    // close button on the right.
    let pad = ui.clone_style().frame_padding;
    let header_y = ui.cursor_pos()[1];
    let header_x = ui.cursor_pos()[0];
    ui.set_cursor_pos([header_x, header_y - pad[1] * 0.5]);
    if ui.small_button("Reset") {
        *reset_layout = true;
    }
    if ui.is_item_hovered() {
        ui.tooltip_text("Reset the menu's position and size");
    }
    ui.same_line();
    ui.set_cursor_pos([header_x, header_y]);
    centered_text(ui, GOLD, "Souls Teleport");
    let close_w = ui.calc_text_size("X")[0] + pad[0] * 2.0;
    ui.same_line_with_pos(ui.window_content_region_max()[0] - close_w);
    ui.set_cursor_pos([ui.cursor_pos()[0], header_y - pad[1] * 0.5]);
    if ui.small_button("X") {
        *keep_open = false;
    }
    ui.separator();
    ui.spacing();

    if !in_session {
        centered_text(ui, TEXT_MUTED, "Not in a co-op session.");
    } else if partners.is_empty() {
        centered_text(ui, TEXT_MUTED, "No other player in this session.");
    } else {
        ui.text_colored(TEXT_MUTED, format!("Players ({})", partners.len()));
        let _disabled = ui.begin_disabled(busy);
        for p in &partners {
            let label = format!("{}##{}", p.character_name, p.steam_id);
            let role_w = ui.calc_text_size(p.role)[0];
            let row_start = ui.cursor_pos();
            if ui.button_with_size(&label, [-1.0, 0.0]) {
                *REQUEST.lock().unwrap() = Some(p.steam_id);
            }
            let hovered = ui.is_item_hovered();
            let after = ui.cursor_pos();
            // Role on the right side of the same button row.
            let x = ui.window_content_region_max()[0] - role_w - pad[0];
            ui.set_cursor_pos([x, row_start[1] + pad[1]]);
            ui.text_colored(role_color(p.role), p.role);
            ui.set_cursor_pos(after);
            if hovered {
                ui.tooltip_text(format!("Steam: {}", p.steam_name));
            }
        }
    }

    if !status.is_empty() {
        ui.spacing();
        ui.separator();
        let color = if status.contains("did not respond") || status.contains("Can't") || status.contains("no longer") {
            ERROR_TEXT
        } else {
            GOLD
        };
        let _wrap = ui.push_text_wrap_pos();
        ui.text_colored(color, &status);
    }
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

fn install_cursor_hooks() {
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

/// Installs the DX12 hooks. Call once from `DllMain`'s worker thread.
pub fn install() {
    match Hudhook::builder().with::<ImguiDx12Hooks>(MenuRenderLoop::default()).build().apply() {
        Ok(()) => {
            logger::log("Menu: DX12 hooks applied.");
            install_cursor_hooks();
            crate::input_block::install();
        }
        Err(e) => logger::error(&format!("Menu: couldn't apply DX12 hooks ({e:?}) - menu unavailable.")),
    }
}
