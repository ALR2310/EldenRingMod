//! Who is in the session, their role, and who can be teleported to.
//!
//! `CSSessionManager.players` is the authority for "in this session" (it
//! works under Seamless Co-op - see README, co-op test 3) and gives each
//! member's SteamID; `WorldChrMan.player_chr_set` gives the character name
//! and `chr_type` of every member the game has a `PlayerIns` for (a far-away
//! partner stays in it, only their position goes stale). The two are joined
//! through `PlayerIns.session_manager_player_entry.steam_id`.
//!
//! Every other session member is listed and can be teleported to, whatever
//! their role - invaders included (user's call, 2026-09-25; the first draft
//! filtered out hostile `chr_type`s). The role is shown next to each name.

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{CSSessionManager, ChrType, WorldChrMan};
use fromsoftware_shared::FromStatic;

/// Last `chr_type` seen for each SteamID, so a member whose `PlayerIns` is
/// gone for a while (loading screen, far away - co-op test 4, 2026-09-24)
/// still shows their role instead of "Unknown".
static KNOWN_ROLES: Mutex<Option<HashMap<u64, ChrType>>> = Mutex::new(None);

pub struct Member {
    pub steam_id: u64,
    pub steam_name: String,
    /// `None` if the game has no `PlayerIns` for this member right now.
    pub character_name: Option<String>,
    /// From their `PlayerIns` if loaded, else the last one remembered in
    /// [KNOWN_ROLES]; `None` if never seen at all.
    pub chr_type: Option<ChrType>,
    pub is_local: bool,
    pub is_host: bool,
}

impl Member {
    /// Anyone but ourselves can be teleported to / gets an answer.
    pub fn is_teleport_target(&self) -> bool {
        !self.is_local
    }

    /// Short role label for the menu.
    pub fn role(&self) -> &'static str {
        if self.is_host {
            return "Host";
        }
        match self.chr_type {
            None => "Unknown",
            // Seamless Co-op reports every connected player as Local.
            Some(ChrType::Local) => "Co-op",
            Some(ChrType::WhitePhantom) => "Cooperator",
            Some(ChrType::Duelist) => "Invader",
            Some(ChrType::BluePhantom) => "Hunter",
            Some(ChrType::BloodyFinger | ChrType::FesteringBloodyFinger | ChrType::BloodyFingerNpc) => {
                "Bloody Finger"
            }
            Some(ChrType::Recusant | ChrType::RecusantNpc) => "Recusant",
            Some(ChrType::Arena) => "Arena",
            Some(_) => "Other",
        }
    }

    /// Character name if known, else Steam name - what messages show.
    pub fn display_name(&self) -> &str {
        self.character_name.as_deref().unwrap_or(&self.steam_name)
    }
}

pub fn character_name(units: &[u16]) -> String {
    let len = units.iter().position(|&c| c == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..len])
}

/// Everyone in `CSSessionManager.players` (empty outside a session), with
/// character name/role filled in where the game has a `PlayerIns` for them.
pub fn members() -> Vec<Member> {
    let Ok(session) = (unsafe { CSSessionManager::instance() }) else {
        return Vec::new();
    };
    let mut members: Vec<Member> = session
        .players
        .iter()
        .filter(|p| p.base.steam_id != 0)
        .map(|p| Member {
            steam_id: p.base.steam_id,
            steam_name: p.base.steam_name.to_string().unwrap_or_default(),
            character_name: None,
            chr_type: None,
            is_local: p.is_local_player,
            is_host: p.is_host,
        })
        .collect();

    let mut known = KNOWN_ROLES.lock().unwrap();
    let known = known.get_or_insert_with(HashMap::new);
    if let Ok(world_chr_man) = unsafe { WorldChrMan::instance() } {
        for player in world_chr_man.player_chr_set.characters() {
            let steam_id = player.session_manager_player_entry.steam_id;
            if let Some(member) = members.iter_mut().find(|m| m.steam_id == steam_id) {
                member.character_name =
                    Some(character_name(&unsafe { player.player_game_data.as_ref() }.character_name));
                member.chr_type = Some(player.chr_ins.chr_type);
                known.insert(steam_id, player.chr_ins.chr_type);
            }
        }
    }
    for member in members.iter_mut().filter(|m| m.chr_type.is_none()) {
        member.chr_type = known.get(&member.steam_id).copied();
    }
    members
}

pub fn own_steam_id(members: &[Member]) -> Option<u64> {
    members.iter().find(|m| m.is_local).map(|m| m.steam_id)
}
