//! Everything under the `[Rune Reward]` ini section: passive rune gain plus
//! milestones ([reward]), and the `Rune.Multiplier` hook ([multiplier]).
//! Grouped by ini section, not by shared code - the two don't call into each
//! other.

pub mod multiplier;
pub mod reward;
