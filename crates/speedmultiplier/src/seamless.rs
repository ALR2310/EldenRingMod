//! Seamless Co-op compatibility (2026-10-03, Nexus report from DrKSolo,
//! reproduced by the user on 1.0.0 and the current build: under Seamless the
//! log shows `animation_speed` set to 3.00, yet nothing in game speeds up).
//!
//! The game reads `CSChrBehaviorModule.animation_speed` in exactly one place
//! (hardware-breakpoint probe, 2026-10-02): the per-frame behavior update
//! `sub_14041DCA0` calls a one-instruction thunk, `sub_140417EA0: jmp
//! <getter>`, and the getter returns `[rcx+0x18]` (rcx = behavior +0x17B0).
//! Seamless (`ersc.dll`) rewrites that thunk's `jmp` to its own code (seen in
//! the in-memory vs exe diff of 2026-10-02: `0x140417EA1`), so whatever we
//! write is never read.
//!
//! Fix: once in game, if the thunk's `jmp` no longer goes to the game's own
//! getter, point it (only its rel32, like SpiritMultiplier's Seamless fix)
//! at a small stub: for the player's or Torrent's behavior it returns the
//! field we set; any other character goes on to whoever hooked it, as
//! before. Re-checked every tick, in case the hook is re-applied.

use std::sync::atomic::{AtomicU64, Ordering};

use common::{codepatch, logger, memscan};

/// `lea rcx,[rdi+17B0h]; call <thunk>; movaps xmm6,xmm0; mulss xmm6,[rdi+15C0h]`
/// in the behavior update (2.7.1.0: 0x14041DD00) - the call is at +7.
const GETTER_CALL_AOB: &str = "48 8D 8F B0 17 00 00 E8 ?? ?? ?? ?? 0F 28 F0 F3 0F 59 B7 C0 15 00 00";
const CALL_OFFSET: usize = 7;

/// Owner `ChrIns` of the player's and Torrent's behavior modules, refreshed
/// every frame by `speed.rs` (0 = none). The stub compares against these.
pub static OWN_CHRS: [AtomicU64; 2] = [AtomicU64::new(0), AtomicU64::new(0)];

pub fn set_own(player: usize, torrent: usize) {
    OWN_CHRS[0].store(player as u64, Ordering::Relaxed);
    OWN_CHRS[1].store(torrent as u64, Ordering::Relaxed);
}

/// The stub (rcx = behavior + 0x17B0, result in xmm0):
/// ```text
/// mov  rax, [rcx-0x17A8]      ; behavior->owner (behavior + 0x8)
/// mov  r10, &OWN_CHRS
/// cmp  rax, [r10]   ; je own  ; the player
/// cmp  rax, [r10+8] ; je own  ; Torrent
/// mov  r10, fallback          ; anyone else: the hook we replaced
/// jmp  r10
/// own: movss xmm0, [rcx+0x18] ; animation_speed
/// ret
/// ```
fn stub(fallback: u64) -> Vec<u8> {
    let mut s = Vec::new();
    s.extend_from_slice(&[0x48, 0x8B, 0x81, 0x58, 0xE8, 0xFF, 0xFF]);
    s.extend_from_slice(&[0x49, 0xBA]);
    s.extend_from_slice(&(OWN_CHRS.as_ptr() as u64).to_le_bytes());
    s.extend_from_slice(&[0x49, 0x3B, 0x02, 0x74, 0x00]); // je patched below
    let je1 = s.len() - 1;
    s.extend_from_slice(&[0x49, 0x3B, 0x42, 0x08, 0x74, 0x00]);
    let je2 = s.len() - 1;
    s.extend_from_slice(&[0x49, 0xBA]);
    s.extend_from_slice(&fallback.to_le_bytes());
    s.extend_from_slice(&[0x41, 0xFF, 0xE2]);
    let own = s.len();
    s.extend_from_slice(&[0xF3, 0x0F, 0x10, 0x41, 0x18, 0xC3]);
    s[je1] = (own - (je1 + 1)) as u8;
    s[je2] = (own - (je2 + 1)) as u8;
    s
}

/// Where the patched thunk lives, and the stub currently installed there.
pub struct GetterFix {
    thunk: Option<*mut u8>,
    /// Thunk target seen last tick - only a change is looked into.
    last_target: u64,
    stub: u64,
    logged_scan_failure: bool,
}

unsafe impl Send for GetterFix {}

impl GetterFix {
    pub const fn new() -> Self {
        Self {
            thunk: None,
            last_target: 0,
            stub: 0,
            logged_scan_failure: false,
        }
    }

    /// Call once per tick while in game.
    pub fn check(&mut self) {
        let thunk = match self.thunk {
            Some(t) => t,
            None => {
                let Some(site) = memscan::find_pattern_in_module(GETTER_CALL_AOB) else {
                    if !std::mem::replace(&mut self.logged_scan_failure, true) {
                        logger::warn("Seamless fix: behavior update not found (AOB) - not needed unless Seamless Co-op is running.");
                    }
                    self.thunk = Some(std::ptr::null_mut());
                    return;
                };
                let call = unsafe { site.add(CALL_OFFSET) };
                let Some(thunk) = codepatch::rel32_target(call).map(|a| a as *mut u8) else {
                    return;
                };
                if unsafe { *thunk } != 0xE9 {
                    logger::warn("Seamless fix: the speed getter thunk isn't a jmp - left alone.");
                    self.thunk = Some(std::ptr::null_mut());
                    return;
                }
                self.thunk = Some(thunk);
                thunk
            }
        };
        if thunk.is_null() {
            return;
        }
        let Some(target) = codepatch::rel32_target(thunk) else {
            return;
        };
        if target == self.last_target || target == self.stub {
            return;
        }
        self.last_target = target;
        // In the game's own image = the original getter: nothing hooks it.
        let in_game = common::diag::module_containing(target as usize)
            .is_some_and(|m| m.name.eq_ignore_ascii_case("eldenring.exe"));
        if in_game {
            return;
        }
        let owner = common::diag::module_containing(target as usize)
            .map(|m| m.name)
            .unwrap_or_else(|| "an unknown module".to_string());
        match codepatch::redirect_rel32(thunk, &stub(target)) {
            Some(stub) => {
                self.stub = stub as u64;
                logger::log(&format!(
                    "Seamless fix: the animation speed getter is hooked by {owner} - the player and Torrent now read their speed past it."
                ));
            }
            None => logger::error("Seamless fix: couldn't redirect the speed getter - speeds won't apply."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stub;

    #[test]
    fn stub_jumps_land_on_the_own_branch() {
        let s = stub(0x1122334455667788);
        // movss xmm0,[rcx+18h]; ret at the end
        let own = s.len() - 6;
        assert_eq!(&s[own..], &[0xF3, 0x0F, 0x10, 0x41, 0x18, 0xC3]);
        // both je rel8 land on it
        for at in [s.iter().position(|&b| b == 0x74).unwrap()] {
            assert_eq!(at + 2 + s[at + 1] as usize, own);
        }
        let second = s.windows(4).position(|w| w == [0x49, 0x3B, 0x42, 0x08]).unwrap() + 4;
        assert_eq!(s[second], 0x74);
        assert_eq!(second + 2 + s[second + 1] as usize, own);
        // fallback address embedded
        assert!(s.windows(8).any(|w| w == 0x1122334455667788u64.to_le_bytes()));
    }
}
