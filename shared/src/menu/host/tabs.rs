//! One tab per mod that exports the menu API (`super::super::schema`):
//! found by looking up `alr_menu_schema_v1` in every loaded module, drawn
//! from its menu file, edited straight in its config file (`toml_edit`, so
//! the user's comments and layout stay) and then reloaded through its
//! `alr_menu_reload_v1`.
//!
//! The config file is re-read every time the menu opens, so edits made by
//! hand (and F5) show up. A value is written when an edit is finished
//! (slider released, text confirmed), not every frame of a drag.

use std::ffi::{CStr, c_char, c_void};

use hudhook::imgui::{TreeNodeFlags, Ui};
use toml_edit::{DocumentMut, Item, Value};

use super::style::{ERROR_TEXT, GOLD_DIM, TEXT_MUTED};
use crate::logger;
use crate::menu::schema::{Field, Group, Kind, Menu, parse_menu};

type TextFn = unsafe extern "C" fn() -> *const c_char;
type ReloadFn = unsafe extern "C" fn() -> i32;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
}

pub struct ModTab {
    /// The mod's DLL file name, for logs.
    dll: String,
    menu: Menu,
    path: Option<String>,
    reload: ReloadFn,
    doc: Option<DocumentMut>,
    /// Problem reading the config file, shown in the tab.
    error: Option<String>,
    /// Outcome of the last save, shown under the tab.
    status: Option<(String, bool)>,
}

unsafe fn text(f: TextFn) -> Option<String> {
    let p = unsafe { f() };
    if p.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

/// Every loaded module exporting the v1 menu API, in load order.
pub fn discover() -> Vec<ModTab> {
    let mut tabs = Vec::new();
    for module in crate::diag::loaded_modules() {
        let base = module.base as *mut c_void;
        let find = |name: &core::ffi::CStr| unsafe { GetProcAddress(base, name.as_ptr() as *const u8) };
        let (schema, path, reload) = (
            find(c"alr_menu_schema_v1"),
            find(c"alr_menu_config_path_v1"),
            find(c"alr_menu_reload_v1"),
        );
        if schema.is_null() || path.is_null() || reload.is_null() {
            continue;
        }
        let (schema, path, reload) = unsafe {
            (
                std::mem::transmute::<*mut c_void, TextFn>(schema),
                std::mem::transmute::<*mut c_void, TextFn>(path),
                std::mem::transmute::<*mut c_void, ReloadFn>(reload),
            )
        };
        let Some(menu_text) = (unsafe { text(schema) }) else {
            continue;
        };
        match parse_menu(&menu_text) {
            Ok(menu) => {
                let mut tab = ModTab {
                    dll: module.name.clone(),
                    menu,
                    path: unsafe { text(path) },
                    reload,
                    doc: None,
                    error: None,
                    status: None,
                };
                tab.refresh();
                tabs.push(tab);
            }
            // A menu file from a newer format, or broken: skip that mod.
            Err(err) => logger::warn(&format!("Menu: {} - menu file not usable, skipped: {err}", module.name)),
        }
    }
    tabs
}

impl ModTab {
    pub fn title(&self) -> &str {
        &self.menu.tab
    }

    /// Re-reads the config file.
    pub fn refresh(&mut self) {
        // The path is only known once the mod loaded its config.
        let Some(path) = &self.path else {
            self.doc = None;
            self.error = Some("The mod hasn't loaded its config yet.".into());
            return;
        };
        match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|t| t.parse::<DocumentMut>().map_err(|e| e.to_string())) {
            Ok(doc) => {
                self.doc = Some(doc);
                self.error = None;
            }
            Err(err) => {
                self.doc = None;
                self.error = Some(format!("Can't read the config file - fix it by hand first.\n{err}"));
            }
        }
    }

    /// Writes the edited file and asks the mod to reload it.
    fn save(&mut self) {
        let (Some(path), Some(doc)) = (&self.path, &self.doc) else {
            return;
        };
        if let Err(err) = std::fs::write(path, doc.to_string()) {
            self.status = Some((format!("Couldn't save: {err}"), false));
            return;
        }
        let ok = unsafe { (self.reload)() } == 0;
        self.status = Some(if ok {
            ("Saved".to_string(), true)
        } else {
            ("Saved, but the mod couldn't load it - see its log".to_string(), false)
        });
        if !ok {
            logger::warn(&format!("Menu: {} rejected its config after a menu edit.", self.dll));
        }
    }

    pub fn draw(&mut self, ui: &Ui) {
        if let Some(err) = &self.error {
            let _wrap = ui.push_text_wrap_pos();
            ui.text_colored(ERROR_TEXT, err);
            return;
        }
        let mut save = false;
        let groups = self.menu.groups.clone();
        for group in &groups {
            save |= self.draw_group(ui, group);
        }
        if save {
            self.save();
        }
        if let Some((text, ok)) = &self.status {
            ui.spacing();
            ui.text_colored(if *ok { GOLD_DIM } else { ERROR_TEXT }, text);
        }
    }

    /// Returns whether an edit was finished this frame (time to save).
    fn draw_group(&mut self, ui: &Ui, group: &Group) -> bool {
        let id = format!("{}##{}", group.label, group.key);
        if !ui.collapsing_header(&id, TreeNodeFlags::DEFAULT_OPEN) {
            return false;
        }
        if !group.description.is_empty() {
            let _wrap = ui.push_text_wrap_pos();
            ui.text_colored(TEXT_MUTED, &group.description);
        }
        if group.list.is_some() {
            ui.text_colored(TEXT_MUTED, format!("Edit [[{}]] in the config file for now.", group.key));
            return false;
        }
        let mut save = false;
        for field in &group.fields {
            save |= self.draw_field(ui, group, field);
        }
        ui.spacing();
        save
    }

    fn value(&self, group: &str, key: &str) -> Option<&Value> {
        self.doc.as_ref()?.get(group)?.get(key)?.as_value()
    }

    /// Replaces a value, keeping the comment / spacing around it.
    fn set(&mut self, group: &str, key: &str, new: Value) {
        let Some(slot) = self.doc.as_mut().and_then(|d| d.get_mut(group)).and_then(|t| t.get_mut(key)) else {
            return;
        };
        match slot.as_value_mut() {
            Some(old) => {
                let decor = old.decor().clone();
                *old = new;
                *old.decor_mut() = decor;
            }
            None => *slot = Item::Value(new),
        }
    }

    fn draw_field(&mut self, ui: &Ui, group: &Group, field: &Field) -> bool {
        let label = format!("{}##{}.{}", field.label, group.key, field.key);
        let Some(current) = self.value(&group.key, &field.key).cloned() else {
            ui.text_colored(TEXT_MUTED, format!("{}: missing from the config file", field.label));
            return false;
        };
        let mut finished = false;
        match &field.kind {
            Kind::Slider { min, max, step } => {
                let mut v = current.as_float().or_else(|| current.as_integer().map(|i| i as f64)).unwrap_or(*min) as f32;
                if ui.slider_config(&label, *min as f32, *max as f32).display_format("%.2f").build(&mut v) {
                    self.set(&group.key, &field.key, Value::from(snap(v as f64, *step)));
                }
                finished = ui.is_item_deactivated_after_edit();
            }
            Kind::Int { min, max } => {
                let mut v = current.as_integer().unwrap_or(*min) as i32;
                if ui.slider_config(&label, *min as i32, *max as i32).build(&mut v) {
                    self.set(&group.key, &field.key, Value::from(v as i64));
                }
                finished = ui.is_item_deactivated_after_edit();
            }
            Kind::Bool => {
                let mut v = current.as_bool().unwrap_or(false);
                if ui.checkbox(&label, &mut v) {
                    self.set(&group.key, &field.key, Value::from(v));
                    finished = true;
                }
            }
            Kind::Key | Kind::Text | Kind::IdList => {
                let mut v = match current.as_str() {
                    Some(s) => s.to_string(),
                    None => current.to_string().trim().to_string(),
                };
                if ui.input_text(&label, &mut v).build() {
                    self.set(&group.key, &field.key, Value::from(v));
                }
                finished = ui.is_item_deactivated_after_edit();
            }
        }
        if !field.description.is_empty() && ui.is_item_hovered() {
            ui.tooltip_text(&field.description);
        }
        finished
    }
}

/// `v` rounded to a multiple of `step`, written with as many decimals as
/// `step` has (so 1.15 stays "1.15", not "1.1500000000000001").
fn snap(v: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return v;
    }
    let decimals = format!("{step}").split('.').nth(1).map_or(0, str::len);
    let rounded = (v / step).round() * step;
    format!("{rounded:.decimals$}").parse().unwrap_or(rounded)
}

#[cfg(test)]
mod tests {
    use super::snap;

    #[test]
    fn snap_rounds_to_the_step() {
        assert_eq!(snap(1.1500000000000001, 0.05), 1.15);
        assert_eq!(snap(1.137, 0.05), 1.15);
        assert_eq!(snap(2.0, 0.05), 2.0);
        assert_eq!(snap(3.3, 0.0), 3.3);
    }
}
