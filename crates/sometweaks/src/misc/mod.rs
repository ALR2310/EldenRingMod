//! Everything under the `[Misc]` ini section: [weight_multiplier],
//! [torrent_anywhere], [unlock_ashes_of_war], [unlock_enchantments], and
//! [warp_anywhere]. Grouped here purely because they share an ini section,
//! not because they share code - none call into each other, and each uses
//! a different technique of its own.
//!
//! `grace_menu` used to live here too, back when it was a small
//! diagnostic test sharing `[Misc]` - now that it's its own full-fledged
//! feature with its own `[Grace Menu]` ini section, it lives at
//! `crate::grace_menu` instead, alongside `regen`/`rune`/`spirit`.

pub mod torrent_anywhere;
pub mod unlock_ashes_of_war;
pub mod unlock_enchantments;
pub mod warp_anywhere;
pub mod weight_multiplier;
