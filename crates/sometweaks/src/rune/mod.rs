//! Everything under the `[Rune Reward]` ini section: passive rune gain plus
//! milestones ([reward]), the `Rune.Multiplier` hook ([multiplier]), and
//! `Rune.KeepOnDeath` ([keep_on_death]). Grouped by ini section, not by
//! shared code - none of the three call into each other.

pub mod keep_on_death;
pub mod multiplier;
pub mod reward;
