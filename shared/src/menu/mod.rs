//! The in-game config menu shared by every mod (design: TODO.md "Menu chỉnh
//! cấu hình trong game").
//!
//! - [schema]: the menu file format (`<Mod>.menu.toml`) and the DLL exports
//!   a mod uses to show up in the menu - always built, every mod needs it.
//! - The host (feature `menu`, pulls in hudhook/ImGui): draws the menu over
//!   the game and finds the mods to show by their exports. v1 (2026-10-02):
//!   the mod that calls [install] hosts; no election between mods yet.
//!   The UI plumbing (theme, fonts, mouse, cursor unpin, mouse blocking) is
//!   a copy of SoulsTeleport's `ui.rs` / `input_block.rs` - SoulsTeleport
//!   keeps its own until a release of it moves over (user, 2026-10-02).

pub mod schema;

#[cfg(feature = "menu")]
mod host;

#[cfg(feature = "menu")]
pub use host::install;
