//! Minimal binding to Steam's `ISteamNetworkingMessages`, resolved at runtime
//! from the `steam_api64.dll` the game itself already loaded and initialized
//! (flat C API exports, so no Steamworks SDK / crate dependency).
//!
//! Used to sync each player's own position to the rest of the session: a
//! Seamless Co-op test (2026-09-24, see README) showed a far-away partner's
//! position is simply not available locally (havok position drops to 0,
//! `PlayerIns` block/position are never filled in for non-main players).

use std::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleA(module_name: *const u8) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, proc_name: *const u8) -> *mut c_void;
}

/// `SteamNetworkingIdentity` (steamnetworkingtypes.h) - 136 bytes: type,
/// payload size, then a 128-byte union. Only the SteamID form is used here.
#[repr(C)]
pub struct SteamNetworkingIdentity {
    kind: i32,
    size: i32,
    data: [u8; 128],
}

// k_ESteamNetworkingIdentityType_SteamID
const IDENTITY_TYPE_STEAM_ID: i32 = 16;

impl SteamNetworkingIdentity {
    pub fn from_steam_id(steam_id: u64) -> Self {
        let mut data = [0u8; 128];
        data[..8].copy_from_slice(&steam_id.to_le_bytes());
        Self { kind: IDENTITY_TYPE_STEAM_ID, size: 8, data }
    }

    fn steam_id(&self) -> Option<u64> {
        (self.kind == IDENTITY_TYPE_STEAM_ID).then(|| u64::from_le_bytes(self.data[..8].try_into().unwrap()))
    }
}

/// Only the leading fields of `SteamNetworkingMessage_t` are read
/// (steamnetworkingtypes.h, unchanged since the struct was introduced):
/// payload pointer, size, connection handle, and `m_identityPeer` - the
/// sender as authenticated by Steam, used to reject packets that claim to be
/// from someone else (review 2026-09-25: the sender used to be taken from our
/// own payload only, which anyone can write). Releasing goes through the
/// exported `SteamAPI_SteamNetworkingMessage_t_Release`, not the struct's own
/// function pointer, so no deeper layout assumption is needed.
#[repr(C)]
struct SteamNetworkingMessage {
    data: *const u8,
    size: i32,
    conn: u32,
    identity_peer: SteamNetworkingIdentity,
}

/// A received message: payload + the sender's SteamID as Steam reports it
/// (`None` if the peer isn't identified by a SteamID).
pub struct Received {
    pub payload: Vec<u8>,
    pub sender: Option<u64>,
}

// k_nSteamNetworkingSend_Reliable | k_nSteamNetworkingSend_AutoRestartBrokenSession.
// Reliable (like SoulsChat's own messages): a teleport is 1 WHERE + 1 HERE,
// so a dropped packet would otherwise just mean waiting out the timeout.
pub const SEND_RELIABLE_AUTO_RESTART: i32 = 8 | 32;

type GetInterfaceFn = unsafe extern "C" fn() -> *mut c_void;
type SendMessageToUserFn = unsafe extern "C" fn(
    iface: *mut c_void,
    identity: *const SteamNetworkingIdentity,
    data: *const u8,
    size: u32,
    flags: i32,
    channel: i32,
) -> i32;
type ReceiveMessagesOnChannelFn =
    unsafe extern "C" fn(iface: *mut c_void, channel: i32, out: *mut *mut SteamNetworkingMessage, max: i32) -> i32;
type AcceptSessionWithUserFn = unsafe extern "C" fn(iface: *mut c_void, identity: *const SteamNetworkingIdentity) -> bool;
type ReleaseFn = unsafe extern "C" fn(message: *mut SteamNetworkingMessage);

pub struct SteamMessages {
    iface: *mut c_void,
    send: SendMessageToUserFn,
    receive: ReceiveMessagesOnChannelFn,
    accept: AcceptSessionWithUserFn,
    release: ReleaseFn,
}

// The interface pointer is process-lifetime and only used from the game's
// own task thread.
unsafe impl Send for SteamMessages {}

fn proc_address(module: *mut c_void, name: &str) -> Option<*mut c_void> {
    let c_name = format!("{name}\0");
    let addr = unsafe { GetProcAddress(module, c_name.as_ptr()) };
    (!addr.is_null()).then_some(addr)
}

impl SteamMessages {
    /// `None` if `steam_api64.dll` isn't loaded, an export is missing, or
    /// Steam hasn't created the interface (e.g. not initialized yet).
    pub fn load() -> Option<Self> {
        let module = unsafe { GetModuleHandleA(c"steam_api64.dll".as_ptr() as *const u8) };
        if module.is_null() {
            return None;
        }
        let get_iface = proc_address(module, "SteamAPI_SteamNetworkingMessages_SteamAPI_v002")?;
        let send = proc_address(module, "SteamAPI_ISteamNetworkingMessages_SendMessageToUser")?;
        let receive = proc_address(module, "SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel")?;
        let accept = proc_address(module, "SteamAPI_ISteamNetworkingMessages_AcceptSessionWithUser")?;
        let release = proc_address(module, "SteamAPI_SteamNetworkingMessage_t_Release")?;
        unsafe {
            let iface = std::mem::transmute::<*mut c_void, GetInterfaceFn>(get_iface)();
            if iface.is_null() {
                return None;
            }
            Some(Self {
                iface,
                send: std::mem::transmute::<*mut c_void, SendMessageToUserFn>(send),
                receive: std::mem::transmute::<*mut c_void, ReceiveMessagesOnChannelFn>(receive),
                accept: std::mem::transmute::<*mut c_void, AcceptSessionWithUserFn>(accept),
                release: std::mem::transmute::<*mut c_void, ReleaseFn>(release),
            })
        }
    }

    /// Returns Steam's `EResult` (1 = OK).
    pub fn send_to(&self, steam_id: u64, payload: &[u8], channel: i32) -> i32 {
        let identity = SteamNetworkingIdentity::from_steam_id(steam_id);
        unsafe {
            (self.send)(
                self.iface,
                &identity,
                payload.as_ptr(),
                payload.len() as u32,
                SEND_RELIABLE_AUTO_RESTART,
                channel,
            )
        }
    }

    /// Accepts (or keeps accepted) the messaging session with a peer - a peer
    /// we haven't sent to yet otherwise has its messages dropped until the
    /// session request is answered. Cheap to repeat.
    pub fn accept(&self, steam_id: u64) {
        let identity = SteamNetworkingIdentity::from_steam_id(steam_id);
        unsafe { (self.accept)(self.iface, &identity) };
    }

    /// Drains every pending message on `channel`, copying each payload (and
    /// its Steam-reported sender) out before releasing it back to Steam.
    pub fn receive_all(&self, channel: i32) -> Vec<Received> {
        const BATCH: usize = 16;
        let mut out = Vec::new();
        loop {
            let mut messages: [*mut SteamNetworkingMessage; BATCH] = [std::ptr::null_mut(); BATCH];
            let count = unsafe { (self.receive)(self.iface, channel, messages.as_mut_ptr(), BATCH as i32) };
            if count <= 0 {
                break;
            }
            for &message in &messages[..count as usize] {
                unsafe {
                    let m = &*message;
                    if !m.data.is_null() && m.size > 0 {
                        out.push(Received {
                            payload: std::slice::from_raw_parts(m.data, m.size as usize).to_vec(),
                            sender: m.identity_peer.steam_id(),
                        });
                    }
                    (self.release)(message);
                }
            }
            if (count as usize) < BATCH {
                break;
            }
        }
        out
    }
}
