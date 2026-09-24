//! Position sync between session members over Steam P2P (see `steam`).
//!
//! Every [BROADCAST_EVERY], each instance sends its own main player's block +
//! block-local position (the only `PlayerIns` whose position fields are
//! actually maintained - see README, co-op test 1/2) to every other
//! `CSSessionManager.players` entry. Received positions are kept per sender,
//! so a warp can target a partner no matter how far away they are.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use eldenring::cs::{CSSessionManager, WorldChrMan};
use fromsoftware_shared::FromStatic;

use crate::steam::SteamMessages;
use crate::warp::Spot;
use common::logger;

/// Our own P2P channel - SoulsChat uses 42; anything else on the same Steam
/// session never sees these messages, and we never see theirs.
pub const CHANNEL: i32 = 0x5354; // "ST"

const BROADCAST_EVERY: Duration = Duration::from_millis(500);

/// A partner's position older than this is treated as unknown (left the
/// session, crashed, stopped sending...).
pub const STALE_AFTER: Duration = Duration::from_secs(5);

const MAGIC: [u8; 4] = *b"STP1";
const NAME_UNITS: usize = 17; // same size as PlayerGameData.character_name
const PACKET_LEN: usize = 4 + 8 + 4 + 4 * 4 + NAME_UNITS * 2;

#[derive(Clone)]
pub struct RemoteSpot {
    pub character_name: String,
    pub spot: Spot,
    pub received: Instant,
}

static REMOTE: Mutex<Option<HashMap<u64, RemoteSpot>>> = Mutex::new(None);

pub struct SessionPeer {
    pub steam_id: u64,
    pub steam_name: String,
    pub is_local: bool,
}

/// Everyone currently in `CSSessionManager.players` (empty when not in a
/// session, or before the singleton exists).
pub fn session_peers() -> Vec<SessionPeer> {
    let Ok(session) = (unsafe { CSSessionManager::instance() }) else {
        return Vec::new();
    };
    session
        .players
        .iter()
        .filter(|p| p.base.steam_id != 0)
        .map(|p| SessionPeer {
            steam_id: p.base.steam_id,
            steam_name: p.base.steam_name.to_string().unwrap_or_default(),
            is_local: p.is_local_player,
        })
        .collect()
}

/// Latest known position of every partner that sent one within [STALE_AFTER].
pub fn fresh_remote_spots() -> Vec<(u64, RemoteSpot)> {
    let guard = REMOTE.lock().unwrap();
    let Some(map) = guard.as_ref() else {
        return Vec::new();
    };
    map.iter()
        .filter(|(_, r)| r.received.elapsed() < STALE_AFTER)
        .map(|(id, r)| (*id, r.clone()))
        .collect()
}

fn own_packet(own_steam_id: u64) -> Option<[u8; PACKET_LEN]> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    let player = world_chr_man.main_player.as_ref()?;
    let pos = &player.block_position;
    let name = unsafe { player.player_game_data.as_ref() }.character_name;

    let mut packet = [0u8; PACKET_LEN];
    let mut at = 0;
    let mut put = |bytes: &[u8]| {
        packet[at..at + bytes.len()].copy_from_slice(bytes);
        at += bytes.len();
    };
    put(&MAGIC);
    put(&own_steam_id.to_le_bytes());
    put(&player.current_block_id.0.to_le_bytes());
    for v in [pos.x, pos.y, pos.z, pos.yaw] {
        put(&v.to_le_bytes());
    }
    for unit in name {
        put(&unit.to_le_bytes());
    }
    Some(packet)
}

fn parse_packet(packet: &[u8]) -> Option<(u64, RemoteSpot)> {
    if packet.len() != PACKET_LEN || packet[..4] != MAGIC {
        return None;
    }
    let u64_at = |i: usize| u64::from_le_bytes(packet[i..i + 8].try_into().unwrap());
    let i32_at = |i: usize| i32::from_le_bytes(packet[i..i + 4].try_into().unwrap());
    let f32_at = |i: usize| f32::from_le_bytes(packet[i..i + 4].try_into().unwrap());
    let steam_id = u64_at(4);
    let spot = Spot {
        block_id: i32_at(12),
        x: f32_at(16),
        y: f32_at(20),
        z: f32_at(24),
        yaw: f32_at(28),
    };
    let units: Vec<u16> = (0..NAME_UNITS)
        .map(|k| u16::from_le_bytes([packet[32 + k * 2], packet[33 + k * 2]]))
        .take_while(|&u| u != 0)
        .collect();
    Some((
        steam_id,
        RemoteSpot {
            character_name: String::from_utf16_lossy(&units),
            spot,
            received: Instant::now(),
        },
    ))
}

/// Per-frame sync step: drain incoming positions every frame, broadcast our
/// own every [BROADCAST_EVERY]. Call from the game's own task thread.
pub struct Syncer {
    steam: SteamMessages,
    last_broadcast: Option<Instant>,
    logged_first_receive: bool,
}

impl Syncer {
    pub fn new(steam: SteamMessages) -> Self {
        Self { steam, last_broadcast: None, logged_first_receive: false }
    }

    pub fn tick(&mut self) {
        for packet in self.steam.receive_all(CHANNEL) {
            let Some((steam_id, remote)) = parse_packet(&packet) else {
                continue;
            };
            if !self.logged_first_receive {
                self.logged_first_receive = true;
                logger::log(&format!(
                    "Sync: first position received from {steam_id} ('{}').",
                    remote.character_name
                ));
            }
            REMOTE.lock().unwrap().get_or_insert_with(HashMap::new).insert(steam_id, remote);
        }

        if self.last_broadcast.is_some_and(|t| t.elapsed() < BROADCAST_EVERY) {
            return;
        }
        self.last_broadcast = Some(Instant::now());

        let peers = session_peers();
        let Some(own) = peers.iter().find(|p| p.is_local) else {
            return; // not in a session
        };
        let Some(packet) = own_packet(own.steam_id) else {
            return; // loading / no main player yet
        };
        for peer in peers.iter().filter(|p| !p.is_local) {
            self.steam.accept(peer.steam_id);
            self.steam.send_to(peer.steam_id, &packet, CHANNEL);
        }
    }
}
