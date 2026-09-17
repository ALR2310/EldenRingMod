//! Shared layer for code that talks to the game's own reflected engine types
//! (`eldenring`/`fromsoftware-shared`), factored out so no single mod owns it
//! and every other mod has to copy-paste from - unlike [`common`] (this
//! workspace's other shared crate), which is deliberately kept free of that
//! dependency for generic mod-loader plumbing (ini config, file logging, AOB
//! scanning) reusable even for a mod targeting a different game entirely.
//!
//! - [`task_hook`]/[`alloc_hook`]: resolve the game's task-registration
//!   function and its global heap allocator via AOB instead of
//!   `fromsoftware-rs`'s version-gated `eldenring::rva::get()` table, so a
//!   mod doesn't panic outright the moment the game updates past whatever
//!   single version that table was last published for.
//! - [`task`]: acquires `CSTaskImp` (also without `rva::get()`) and wraps a
//!   per-frame closure with `catch_unwind`, so a panic there can't unwind
//!   into the game's own (non-Rust) call stack.
//! - [`player`]: the "is the player actually in the game world yet" gate
//!   that `GameDataMan`/`SoloParamRepository`-touching features need before
//!   their data is safe to read.
//! - [`reload`]: watches a `ReloadKey` ini hotkey and reloads config,
//!   independent of any feature module.
//!
//! First extracted (2026-09-14) from `autoregen` (which had already done the
//! AOB-resilience work) and `sometweaks`/`risearcher` (which both carried an
//! older, still-`rva::get()`-gated `task.rs`/`player.rs`/`reload.rs` of their
//! own) once a new mod needed the same thing a third/fourth time.

pub mod alloc_hook;
pub mod player;
pub mod reload;
pub mod task;
pub mod task_hook;
