//! The menu host: the ImGui window over the game (hudhook, DX12), opened
//! with a hotkey, one tab per mod found by `tabs::discover`. Render-loop
//! scaffolding and hooks copied from SoulsTeleport's `ui.rs` (see `style`).
//!
//! v1 leaves out what SoulsTeleport's menu has on top: saving the window's
//! position/size and `MenuScale`.

mod input_block;
mod style;
mod tabs;

use std::sync::atomic::{AtomicBool, Ordering};

use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::imgui::{Condition, Context, FontConfig, FontGlyphRanges, FontSource, Io, MouseButton, Ui};
use hudhook::windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use hudhook::{BeforeWndProc, Hudhook, ImguiRenderLoop, MessageFilter, RenderContext};

use crate::logger;

pub(crate) static MENU_OPEN: AtomicBool = AtomicBool::new(false);

const WINDOW_SIZE_AT_1080P: [f32; 2] = [560.0, 640.0];
const WINDOW_POS_AT_1080P: [f32; 2] = [60.0, 60.0];

#[derive(Default)]
struct MenuRenderLoop {
    /// Unscaled style, re-derived on every scale change (see SoulsTeleport).
    base_style: Option<hudhook::imgui::Style>,
    scale: f32,
    relayout: bool,
    buttons_down: [bool; 2],
    /// `None` until the menu is first opened.
    tabs: Option<Vec<tabs::ModTab>>,
    was_open: bool,
}

impl ImguiRenderLoop for MenuRenderLoop {
    fn initialize<'a>(&'a mut self, ctx: &mut Context, _render_context: &'a mut dyn RenderContext) {
        ctx.set_ini_filename(None);
        let main = FontSource::TtfData {
            data: style::EMBEDDED_FONT,
            size_pixels: style::FONT_RASTER_BASE,
            config: Some(FontConfig {
                glyph_ranges: FontGlyphRanges::from_slice(&style::GLYPH_RANGES),
                ..FontConfig::default()
            }),
        };
        match style::find_cjk_font() {
            Some((_, bytes)) => {
                ctx.fonts().add_font(&[
                    main,
                    FontSource::TtfData {
                        data: &bytes,
                        size_pixels: style::FONT_RASTER_BASE,
                        config: Some(FontConfig {
                            glyph_ranges: FontGlyphRanges::chinese_simplified_common(),
                            ..FontConfig::default()
                        }),
                    },
                ]);
            }
            None => {
                ctx.fonts().add_font(&[main]);
            }
        }
        style::apply_theme(ctx.style_mut());
        self.base_style = Some(*ctx.style());
        logger::log("Menu: ImGui initialized.");
    }

    fn before_render<'a>(&'a mut self, ctx: &mut Context, _render_context: &'a mut dyn RenderContext) {
        let open = MENU_OPEN.load(Ordering::Relaxed);
        ctx.io_mut().mouse_draw_cursor = open;
        if open {
            style::feed_mouse(ctx, &mut self.buttons_down);
        } else if self.buttons_down != [false; 2] {
            self.buttons_down = [false; 2];
            ctx.io_mut().add_mouse_button_event(MouseButton::Left, false);
            ctx.io_mut().add_mouse_button_event(MouseButton::Right, false);
        }

        let scale = style::scale_for(ctx.io().display_size);
        if (scale - self.scale).abs() > 0.01 {
            if let Some(base) = self.base_style {
                let mut s = base;
                s.scale_all_sizes(scale);
                s.mouse_cursor_scale = base.mouse_cursor_scale * scale;
                *ctx.style_mut() = s;
            }
            ctx.io_mut().font_global_scale = style::FONT_AT_1080P * scale / style::FONT_RASTER_BASE;
            self.scale = scale;
            self.relayout = true;
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        let open = MENU_OPEN.load(Ordering::Relaxed);
        // Opening: find the mods (first time) or re-read their config files.
        if open && !self.was_open {
            match &mut self.tabs {
                None => {
                    let found = tabs::discover();
                    logger::log(&format!("Menu: {} mod(s) with a menu tab.", found.len()));
                    self.tabs = Some(found);
                }
                Some(tabs) => tabs.iter_mut().for_each(|t| t.refresh()),
            }
        }
        self.was_open = open;
        if !open {
            return;
        }

        let scale = if self.scale > 0.0 { self.scale } else { 1.0 };
        let cond = if std::mem::take(&mut self.relayout) { Condition::Always } else { Condition::FirstUseEver };
        let mut keep_open = true;
        let tabs = self.tabs.get_or_insert_with(Vec::new);
        ui.window("##alr_mod_menu")
            .title_bar(false)
            .size(WINDOW_SIZE_AT_1080P.map(|v| v * scale), cond)
            .position(WINDOW_POS_AT_1080P.map(|v| v * scale), cond)
            .collapsible(false)
            .build(|| draw_window(ui, tabs, &mut keep_open));
        if !keep_open {
            MENU_OPEN.store(false, Ordering::Relaxed);
        }
    }

    fn before_wnd_proc(&self, _hwnd: HWND, umsg: u32, _wparam: WPARAM, _lparam: LPARAM) -> BeforeWndProc {
        // Same as SoulsTeleport: while open, the mouse comes only from
        // `style::feed_mouse` (hudhook's raw-input path never gets a valid
        // position in this game).
        const WM_INPUT: u32 = 0x00FF;
        const WM_MOUSEFIRST: u32 = 0x0200;
        const WM_MOUSELAST: u32 = 0x020E;
        let mouse_msg = umsg == WM_INPUT || (WM_MOUSEFIRST..=WM_MOUSELAST).contains(&umsg);
        if mouse_msg && MENU_OPEN.load(Ordering::Relaxed) { BeforeWndProc::Break } else { BeforeWndProc::Continue }
    }

    fn message_filter(&self, _io: &Io) -> MessageFilter {
        if MENU_OPEN.load(Ordering::Relaxed) { MessageFilter::InputAll } else { MessageFilter::empty() }
    }
}

fn draw_window(ui: &Ui, tabs: &mut [tabs::ModTab], keep_open: &mut bool) {
    // Header: title centered, close button on the right.
    let pad = ui.clone_style().frame_padding;
    let title = "Mod Settings";
    let header_y = ui.cursor_pos()[1];
    let avail = ui.content_region_avail()[0];
    let width = ui.calc_text_size(title)[0];
    ui.set_cursor_pos([ui.cursor_pos()[0] + ((avail - width) * 0.5).max(0.0), header_y]);
    ui.text_colored(style::GOLD, title);
    let close_w = ui.calc_text_size("X")[0] + pad[0] * 2.0;
    ui.same_line_with_pos(ui.window_content_region_max()[0] - close_w);
    ui.set_cursor_pos([ui.cursor_pos()[0], header_y - pad[1] * 0.5]);
    if ui.small_button("X") {
        *keep_open = false;
    }
    ui.separator();

    if tabs.is_empty() {
        ui.text_colored(style::TEXT_MUTED, "No mod with settings found.");
        return;
    }
    if let Some(_bar) = ui.tab_bar("##alr_mod_tabs") {
        for (i, tab) in tabs.iter_mut().enumerate() {
            if let Some(_item) = ui.tab_item(format!("{}##tab{i}", tab.title())) {
                ui.child_window(format!("##tab_body{i}")).build(|| tab.draw(ui));
            }
        }
    }
}

/// Installs the menu: DX12 hooks, cursor unpin, mouse blocking, and a game
/// task toggling it on `menu_key()` (re-read every frame, so a config
/// reload can change it). Call once, from `DllMain`'s worker thread.
pub fn install<K>(menu_key: K)
where
    K: Fn() -> String + Send + 'static,
{
    match Hudhook::builder().with::<ImguiDx12Hooks>(MenuRenderLoop::default()).build().apply() {
        Ok(()) => {
            logger::log("Menu: DX12 hooks applied.");
            style::install_cursor_hooks();
            input_block::install();
        }
        Err(e) => {
            logger::error(&format!("Menu: couldn't apply DX12 hooks ({e:?}) - menu unavailable."));
            return;
        }
    }

    const VK_F10: i32 = 0x79;
    std::thread::spawn(move || {
        let cs_task = crate::task::wait_for_cs_task();
        crate::task::run_recurring_safe(
            cs_task,
            "Menu",
            eldenring::cs::CSTaskGroupIndex::FrameBegin,
            move |_data: &eldenring::fd4::FD4TaskData| {
                let key = crate::input::parse_virtual_key(&menu_key(), VK_F10);
                if eldenring::util::input::is_key_pressed(key) {
                    MENU_OPEN.fetch_xor(true, Ordering::Relaxed);
                }
            },
        );
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    });
}
