//! On-demand position exchange over Steam P2P (see `steam`), replacing the
//! test-phase broadcast (every player sent its position to the whole session
//! every 500 ms - see README).
//!
//! - Requester: [Net::request] sends `WHERE` to one partner's SteamID and
//!   waits up to [REPLY_TIMEOUT].
//! - Responder: on `WHERE` from anyone else in the session,
//!   replies `HERE` (right away, or once back in the world if it arrived
//!   mid-load) with our own main player's block + block-local position
//!   (the only `PlayerIns` whose position fields are maintained) and
//!   character name. Anyone else - not in the session, an invader - gets no
//!   answer, so a position is only ever sent when a partner asks for it.

use std::time::{Duration, Instant};

use eldenring::cs::{CSSessionManager, ProtocolState, WorldChrMan};
use fromsoftware_shared::FromStatic;

use crate::party::{self, Member};
use crate::steam::SteamMessages;
use crate::warp::Spot;
use common::logger;

/// Our own P2P channel - SoulsChat uses 42; nothing else on the same Steam
/// session sees these messages, and we never see theirs.
const CHANNEL: i32 = 0x5354; // "ST"

// Generous on purpose: the partner may be mid-load (fast travel, respawn) and
// only answers once back in the world. Was 2 s in the first draft - too short.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// A peer's messages are dropped by Steam until we accept its session (or
/// send to it first) - re-accepted for every partner this often so a
/// partner's first `WHERE` isn't lost.
const ACCEPT_EVERY: Duration = Duration::from_secs(2);

/// How long we must have been continuously in the world before answering a
/// `WHERE`. Co-op test 5 (2026-09-25): warping to a player who is still on a
/// loading screen (fast travel to a grace) kicked the warping player back to
/// the title screen, both ways round - while loading, `WorldChrMan` can
/// still hold the old main player, so the old code answered with a position
/// in a map the answerer was leaving. Warping to someone done loading works.
const SETTLE_TIME: Duration = Duration::from_secs(2);

const MAGIC_WHERE: [u8; 4] = *b"STW1";
const MAGIC_HERE: [u8; 4] = *b"STH1";
const NAME_UNITS: usize = 17; // same size as PlayerGameData.character_name
const WHERE_LEN: usize = 4 + 8 + 4;
const HERE_LEN: usize = 4 + 8 + 4 + 4 + 4 * 4 + NAME_UNITS * 2;

pub enum Event {
    Arrived { character_name: String, spot: Spot },
    TimedOut { target_name: String },
}

struct Pending {
    target: u64,
    target_name: String,
    request_id: u32,
    sent: Instant,
}

/// A partner's `WHERE` that arrived while we weren't in the world (loading,
/// respawning) - answered as soon as we are, until it would be too late for
/// the asker anyway.
struct Deferred {
    asker: u64,
    asker_name: String,
    request_id: u32,
    received: Instant,
}

pub struct Net {
    steam: SteamMessages,
    /// Since when we've been continuously "in the world" (see
    /// [in_world_now]); `None` while loading / in menus.
    in_world_since: Option<Instant>,
    pending: Option<Pending>,
    deferred: Vec<Deferred>,
    next_request_id: u32,
    last_accept: Option<Instant>,
}

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new(magic: [u8; 4]) -> Self {
        Self { buf: magic.to_vec() }
    }
    fn put(&mut self, bytes: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(bytes);
        self
    }
}

fn u32_at(p: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(p[i..i + 4].try_into().unwrap())
}
fn u64_at(p: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(p[i..i + 8].try_into().unwrap())
}
fn f32_at(p: &[u8], i: usize) -> f32 {
    f32::from_le_bytes(p[i..i + 4].try_into().unwrap())
}

/// Loaded into the world right now: a main player exists, no move-map is
/// pending (`GameMan.warp_requested`), and the session isn't in one of its
/// (re)load phases. Any one alone isn't enough - see [SETTLE_TIME].
fn in_world_now() -> bool {
    if common::player::main_player_chr_ins_ptr().is_none() || crate::warp::warp_pending() {
        return false;
    }
    match unsafe { CSSessionManager::instance() } {
        Ok(session) => !matches!(
            session.protocol_state,
            ProtocolState::WaitInitData
                | ProtocolState::WaitReloadWait
                | ProtocolState::WaitReload
                | ProtocolState::WaitReload2
                | ProtocolState::WaitReentryToMap
        ),
        Err(_) => true,
    }
}

/// Our main player's position + character name, `None` while loading / in
/// the menus. Only called once [Net::settled].
fn own_here(own_steam_id: u64, request_id: u32) -> Option<Vec<u8>> {
    let world_chr_man = unsafe { WorldChrMan::instance() }.ok()?;
    let player = world_chr_man.main_player.as_ref()?;
    let pos = &player.block_position;
    let name = unsafe { player.player_game_data.as_ref() }.character_name;
    let mut w = Writer::new(MAGIC_HERE);
    w.put(&own_steam_id.to_le_bytes())
        .put(&request_id.to_le_bytes())
        .put(&player.current_block_id.0.to_le_bytes());
    for v in [pos.x, pos.y, pos.z, pos.yaw] {
        w.put(&v.to_le_bytes());
    }
    for unit in name {
        w.put(&unit.to_le_bytes());
    }
    Some(w.buf)
}

impl Net {
    pub fn new(steam: SteamMessages) -> Self {
        Self { steam, in_world_since: None, pending: None, deferred: Vec::new(), next_request_id: 1, last_accept: None }
    }

    /// Asks `target` for its position. Replaces any request still waiting.
    pub fn request(&mut self, own_steam_id: u64, target: &Member) {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        let mut w = Writer::new(MAGIC_WHERE);
        w.put(&own_steam_id.to_le_bytes()).put(&request_id.to_le_bytes());
        let result = self.steam.send_to(target.steam_id, &w.buf, CHANNEL);
        logger::log(&format!(
            "WHERE -> '{}' ({}) request {request_id}, send result {result}.",
            target.display_name(),
            target.steam_id
        ));
        self.pending = Some(Pending {
            target: target.steam_id,
            target_name: target.display_name().to_string(),
            request_id,
            sent: Instant::now(),
        });
    }

    /// Per-frame step, from the game's own task thread: answers partners'
    /// `WHERE`s, and returns the outcome of our own pending request, if any.
    pub fn tick(&mut self) -> Option<Event> {
        if in_world_now() {
            self.in_world_since.get_or_insert_with(Instant::now);
        } else {
            self.in_world_since = None;
        }
        let packets = self.steam.receive_all(CHANNEL);
        let accept_due = self.last_accept.is_none_or(|t| t.elapsed() >= ACCEPT_EVERY);
        self.deferred.retain(|d| d.received.elapsed() < REPLY_TIMEOUT);
        // Only walk the session/character lists when something needs them -
        // this runs every frame.
        let needs_members = accept_due || !packets.is_empty() || !self.deferred.is_empty();
        let members = if needs_members { party::members() } else { Vec::new() };

        if !self.deferred.is_empty() {
            self.flush_deferred(&members);
        }

        if accept_due {
            self.last_accept = Some(Instant::now());
            for m in members.iter().filter(|m| m.is_teleport_target()) {
                self.steam.accept(m.steam_id);
            }
        }

        let mut event = None;
        for packet in packets {
            if packet.len() == WHERE_LEN && packet[..4] == MAGIC_WHERE {
                self.answer_where(&members, u64_at(&packet, 4), u32_at(&packet, 12));
            } else if packet.len() == HERE_LEN && packet[..4] == MAGIC_HERE {
                if let Some(e) = self.take_here(&packet) {
                    event = Some(e);
                }
            }
        }

        if event.is_none() {
            if let Some(p) = self.pending.as_ref().filter(|p| p.sent.elapsed() >= REPLY_TIMEOUT) {
                logger::log(&format!("No HERE from '{}' within {:?}.", p.target_name, REPLY_TIMEOUT));
                event = Some(Event::TimedOut { target_name: p.target_name.clone() });
                self.pending = None;
            }
        }
        event
    }

    fn answer_where(&mut self, members: &[Member], from: u64, request_id: u32) {
        let Some(asker) = members.iter().find(|m| m.steam_id == from) else {
            logger::log(&format!("Ignored WHERE from {from}: not in this session."));
            return;
        };
        let Some(own) = party::own_steam_id(members) else {
            return;
        };
        let reply = if self.settled() { own_here(own, request_id) } else { None };
        let Some(reply) = reply else {
            logger::log(&format!(
                "WHERE from '{}': not in the world yet, will answer once back in.",
                asker.display_name()
            ));
            self.deferred.push(Deferred {
                asker: from,
                asker_name: asker.display_name().to_string(),
                request_id,
                received: Instant::now(),
            });
            return;
        };
        let result = self.steam.send_to(from, &reply, CHANNEL);
        logger::log(&format!(
            "HERE -> '{}' request {request_id}, send result {result}.",
            asker.display_name()
        ));
    }

    /// Answers every deferred `WHERE` once we're back in the world.
    fn flush_deferred(&mut self, members: &[Member]) {
        let Some(own) = party::own_steam_id(members) else {
            return;
        };
        if !self.settled() {
            return;
        }
        let mut still_waiting = Vec::new();
        for d in std::mem::take(&mut self.deferred) {
            match own_here(own, d.request_id) {
                Some(reply) => {
                    let result = self.steam.send_to(d.asker, &reply, CHANNEL);
                    logger::log(&format!(
                        "HERE -> '{}' request {} (deferred {:.1}s), send result {result}.",
                        d.asker_name,
                        d.request_id,
                        d.received.elapsed().as_secs_f32()
                    ));
                }
                None => still_waiting.push(d),
            }
        }
        self.deferred = still_waiting;
    }

    fn settled(&self) -> bool {
        self.in_world_since.is_some_and(|t| t.elapsed() >= SETTLE_TIME)
    }

    fn take_here(&mut self, p: &[u8]) -> Option<Event> {
        let from = u64_at(p, 4);
        let request_id = u32_at(p, 12);
        let pending = self.pending.as_ref()?;
        if pending.target != from || pending.request_id != request_id {
            return None; // late answer to an older / replaced request
        }
        let spot = Spot {
            block_id: u32_at(p, 16) as i32,
            x: f32_at(p, 20),
            y: f32_at(p, 24),
            z: f32_at(p, 28),
            yaw: f32_at(p, 32),
        };
        let units: Vec<u16> = (0..NAME_UNITS)
            .map(|k| u16::from_le_bytes([p[36 + k * 2], p[37 + k * 2]]))
            .collect();
        let character_name = party::character_name(&units);
        logger::log(&format!(
            "HERE <- '{character_name}' request {request_id} after {:.0} ms: block {:#010X} local ({:.2}, {:.2}, {:.2}).",
            pending.sent.elapsed().as_secs_f32() * 1000.0,
            spot.block_id,
            spot.x,
            spot.y,
            spot.z
        ));
        self.pending = None;
        Some(Event::Arrived { character_name, spot })
    }
}
