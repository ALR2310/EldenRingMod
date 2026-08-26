//! Everything under the `[Spirit]` ini section: [color], [regen],
//! [summon_anywhere], and [summon_count]. Grouped here purely because they
//! share an ini section, not because they share code - each is
//! independent.

pub mod color;
pub mod regen;
pub mod summon_anywhere;
pub mod summon_count;
