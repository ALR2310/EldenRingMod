//! Shows text in the game's own top-of-screen system announcement banner -
//! the same widget used for things like "Autosaving..." - so a mod can give
//! visual, in-game confirmation of something (e.g. "config reloaded")
//! without the player needing to check a log file.
//!
//! Ported from `autoregen::regen`'s `show_announcement` (2026-09-10) once a
//! second mod ([`dropmultiplier`](../../crates/dropmultiplier)) wanted the
//! same confirmation on `ReloadKey` press.

use eldenring::cs::{AnnounceNotification, CSMenuManImp, MenuString};
use eldenring::dltx::DLString;
use fromsoftware_shared::FromStatic;

/// Queues `text` onto `CSMenuMan`'s `FeSystemAnnounceViewModel`, so the
/// game's existing fade-in/scroll/fade-out playback handles displaying and
/// dismissing it, no timer of our own needed. No-op (silently) if
/// `CSMenuMan` isn't resolved yet, the runtime heap allocator (see
/// [`crate::alloc_hook`]) isn't resolved yet, or the string fails to encode
/// - a missed banner isn't worth a log line, callers that also log to a
/// file already have that covered.
pub fn show_announcement(text: &str) {
    let Ok(menu_man) = (unsafe { CSMenuManImp::instance_mut() }) else {
        return;
    };
    let Some(allocator) = crate::alloc_hook::runtime_heap_allocator() else {
        return;
    };
    let Ok(allocated_string) = DLString::from_str(text, allocator) else {
        return;
    };
    menu_man.system_announce_view_model.notifications.push_back(AnnounceNotification {
        is_active: true,
        message: MenuString {
            static_string: std::ptr::null(),
            allocated_string,
        },
    });
}
