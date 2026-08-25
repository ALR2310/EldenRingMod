//! Everything under the `[Misc]` ini section: [weight_multiplier] and
//! [torrent_anywhere]. Grouped here purely because they share an ini
//! section, not because they share code - neither calls into the other, and
//! each uses a different anchor/patch technique of its own.

pub mod torrent_anywhere;
pub mod weight_multiplier;
