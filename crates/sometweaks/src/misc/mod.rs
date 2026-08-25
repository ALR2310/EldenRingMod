//! Everything under the `[Misc]` ini section: [weight_multiplier],
//! [torrent_anywhere], [unlock_ashes_of_war], and [unlock_enchantments].
//! Grouped here purely because they share an ini section, not because they
//! share code - none call into each other, and each uses a different
//! technique of its own.

pub mod torrent_anywhere;
pub mod unlock_ashes_of_war;
pub mod unlock_enchantments;
pub mod weight_multiplier;
