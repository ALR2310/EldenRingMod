//! `SpeedMultiplier.toml` (2026-10-02, replaces `SpeedMultiplier.ini`):
//! loaded through `common::toml_config`, so the per-SpEffect overrides can
//! be a list of tables instead of numbered ini keys. Key names stay
//! PascalCase, as in the ini.
//!
//! Tables `[Player]` / `[Torrent]` (2026-10-02, the user's layout - before
//! release it was one `[Speed]` with `Player*` / `Torrent` keys).
//! `General.ConfigVersion` is the file format version for future
//! migrations ([STEPS]); 1 is the first released format.
//!
//! `[[Override]]` (2026-10-02, Nexus request from Lwingr): `SpEffect` is
//! one id or a list (the player has ANY of them -> active), plus any
//! `Player.*` / `Torrent.*` key as a dotted key (`Player.Roll = 1.5`).
//! Every active override applies, top to bottom, so a later one wins on a
//! key both set - see [Config::effective_speed].
//!
//! `Default` must match the embedded template - the test below checks it.

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Deserializer};

use common::toml_config::{LoadReport, Migration, Step, TomlConfig};

/// Embedded verbatim at compile time - single source of truth for the
/// default file.
pub const TEMPLATE: &str = include_str!("../SpeedMultiplier.toml");

/// Format migration steps for `General.ConfigVersion` (see
/// `common::toml_config`): `STEPS[i]` takes a file from version `i + 1` to
/// `i + 2`. Version 1 = the first released TOML format (1.1.0). Append
/// only - a released step must never change.
const STEPS: &[Step] = &[];

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Config {
    pub general: General,
    pub player: Player,
    pub torrent: Torrent,
    pub logging: Logging,
    #[serde(rename = "Override")]
    pub overrides: Vec<Override>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct General {
    /// File format version, managed by the mod.
    pub config_version: u32,
    #[serde(deserialize_with = "common::toml_config::key_name")]
    pub reload_key: String,
    pub reload_banner: bool,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Player {
    pub all: f32,
    pub walk: f32,
    pub run: f32,
    pub sneak: f32,
    pub jump: f32,
    pub roll: f32,
    pub attack: f32,
    pub critical: f32,
    pub skill: f32,
    pub cast: f32,
    pub item: f32,
    pub other: f32,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Torrent {
    pub all: f32,
    pub walk: f32,
    pub run: f32,
    pub jump: f32,
    pub other: f32,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct Logging {
    pub log_file: bool,
    pub speed_probe: bool,
    pub effect_probe: bool,
}

/// `[Player]` + `[Torrent]` after overrides - what `speed.rs` applies.
#[derive(Debug, Clone, PartialEq)]
pub struct Speeds {
    pub player: Player,
    pub torrent: Torrent,
}

/// One `[[Override]]`: the keys it sets while the player has any of
/// `sp_effect`. No `#[serde(default)]` on the struct - an override without
/// `SpEffect` is an error, not one that is always on.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
pub struct Override {
    #[serde(rename = "SpEffect", deserialize_with = "sp_effect_ids")]
    pub sp_effect: Vec<i32>,
    #[serde(default)]
    pub player: PlayerOverride,
    #[serde(default)]
    pub torrent: TorrentOverride,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct PlayerOverride {
    pub all: Option<f32>,
    pub walk: Option<f32>,
    pub run: Option<f32>,
    pub sneak: Option<f32>,
    pub jump: Option<f32>,
    pub roll: Option<f32>,
    pub attack: Option<f32>,
    pub critical: Option<f32>,
    pub skill: Option<f32>,
    pub cast: Option<f32>,
    pub item: Option<f32>,
    pub other: Option<f32>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default, deny_unknown_fields, rename_all = "PascalCase")]
pub struct TorrentOverride {
    pub all: Option<f32>,
    pub walk: Option<f32>,
    pub run: Option<f32>,
    pub jump: Option<f32>,
    pub other: Option<f32>,
}

impl Override {
    /// Writes every key this override sets into `speeds`.
    fn apply_to(&self, speeds: &mut Speeds) {
        let (o, p) = (&self.player, &mut speeds.player);
        let pairs = [
            (o.all, &mut p.all),
            (o.walk, &mut p.walk),
            (o.run, &mut p.run),
            (o.sneak, &mut p.sneak),
            (o.jump, &mut p.jump),
            (o.roll, &mut p.roll),
            (o.attack, &mut p.attack),
            (o.critical, &mut p.critical),
            (o.skill, &mut p.skill),
            (o.cast, &mut p.cast),
            (o.item, &mut p.item),
            (o.other, &mut p.other),
            (self.torrent.all, &mut speeds.torrent.all),
            (self.torrent.walk, &mut speeds.torrent.walk),
            (self.torrent.run, &mut speeds.torrent.run),
            (self.torrent.jump, &mut speeds.torrent.jump),
            (self.torrent.other, &mut speeds.torrent.other),
        ];
        for (value, slot) in pairs {
            if let Some(value) = value {
                *slot = value;
            }
        }
    }
}

impl Config {
    /// `[Player]` / `[Torrent]` with every override whose SpEffects
    /// `has_sp_effect` finds applied on top, in file order, plus the
    /// indices of those overrides.
    pub fn effective_speed(&self, has_sp_effect: impl Fn(i32) -> bool) -> (Speeds, Vec<usize>) {
        let mut speeds = Speeds {
            player: self.player.clone(),
            torrent: self.torrent.clone(),
        };
        let mut active = Vec::new();
        for (index, o) in self.overrides.iter().enumerate() {
            if o.sp_effect.iter().any(|&id| has_sp_effect(id)) {
                o.apply_to(&mut speeds);
                active.push(index);
            }
        }
        (speeds, active)
    }
}

/// `SpEffect = 1234`, `SpEffect = [1234, 1235]`; ids also accepted as
/// strings (`"1234"`). An empty list is an error - it could never match.
fn sp_effect_ids<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<i32>, D::Error> {
    use serde::de::{Error, SeqAccess, Visitor};

    fn id<E: Error>(value: i64) -> Result<i32, E> {
        i32::try_from(value)
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| E::custom(format!("SpEffect id {value} is not a valid id")))
    }
    fn id_str<E: Error>(value: &str) -> Result<i32, E> {
        let n: i64 = value
            .trim()
            .parse()
            .map_err(|_| E::custom(format!("SpEffect id \"{value}\" is not a number")))?;
        id(n)
    }

    struct Ids;
    impl<'de> Visitor<'de> for Ids {
        type Value = Vec<i32>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a SpEffect id or a list of ids, e.g. 1234 or [1234, 1235]")
        }
        fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(vec![id(v)?])
        }
        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(vec![id_str(v)?])
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum One {
                Num(i64),
                Text(String),
            }
            let mut ids = Vec::new();
            while let Some(one) = seq.next_element::<One>()? {
                ids.push(match one {
                    One::Num(n) => id(n)?,
                    One::Text(t) => id_str(&t)?,
                });
            }
            if ids.is_empty() {
                return Err(A::Error::custom("SpEffect = [] is empty - list at least one id"));
            }
            Ok(ids)
        }
    }
    deserializer.deserialize_any(Ids)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General::default(),
            player: Player::default(),
            torrent: Torrent::default(),
            logging: Logging::default(),
            overrides: Vec::new(),
        }
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            config_version: 1,
            reload_key: "F5".to_string(),
            reload_banner: true,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self {
            all: 1.0,
            walk: 1.2,
            run: 1.2,
            sneak: 1.2,
            jump: 1.0,
            roll: 1.1,
            attack: 1.2,
            critical: 1.0,
            skill: 1.2,
            cast: 1.2,
            item: 1.0,
            other: 1.0,
        }
    }
}

impl Default for Torrent {
    fn default() -> Self {
        Self {
            all: 1.0,
            walk: 1.0,
            run: 1.3,
            jump: 1.0,
            other: 1.0,
        }
    }
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            log_file: true,
            speed_probe: false,
            effect_probe: false,
        }
    }
}

const MIGRATION: Migration = Migration {
    version_key: "General.ConfigVersion",
    steps: STEPS,
    keep: &["Override"],
};

static CONFIG: OnceLock<TomlConfig<Config>> = OnceLock::new();

/// Loads (creating / bringing in line with the template) `path`. Call
/// once, first thing in `DllMain`'s thread; the report is for logging once
/// the logger is up.
pub fn init(path: &str) -> LoadReport {
    let (config, report) = TomlConfig::load_or_create(
        path,
        TEMPLATE,
        MIGRATION,
    );
    let _ = CONFIG.set(config);
    report
}

/// The config in use - take it once per tick.
pub fn get() -> Arc<Config> {
    match CONFIG.get() {
        Some(config) => config.get(),
        None => Arc::new(Config::default()),
    }
}

pub fn reload() -> Result<(), String> {
    match CONFIG.get() {
        Some(config) => config.reload(),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_matches_default() {
        let parsed: Config = toml::from_str(TEMPLATE).expect("template parses");
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn missing_keys_fall_back_to_default() {
        let parsed: Config = toml::from_str("[Player]\nRoll = 2.0\n").unwrap();
        assert_eq!(parsed.player.roll, 2.0);
        assert_eq!(parsed.torrent.run, 1.3);
        assert!(parsed.logging.log_file);
    }

    #[test]
    fn misspelled_key_is_an_error_with_its_line() {
        let err = toml::from_str::<Config>("[Player]\nRol = 2.0\n").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Rol"), "{msg}");
        assert!(msg.contains("line 2"), "{msg}");
    }

    #[test]
    fn reload_key_takes_a_name_or_a_number() {
        let parsed: Config = toml::from_str("[General]\nReloadKey = \"0x74\"\n").unwrap();
        assert_eq!(parsed.general.reload_key, "0x74");
        let parsed: Config = toml::from_str("[General]\nReloadKey = 116\n").unwrap();
        assert_eq!(parsed.general.reload_key, "116");
        // A TOML hex literal is an integer too (user test, 2026-10-02).
        let parsed: Config = toml::from_str("[General]\nReloadKey = 0x75\n").unwrap();
        assert_eq!(common::input::parse_virtual_key(&parsed.general.reload_key, 0x74), 0x75);
    }

    fn has(ids: &'static [i32]) -> impl Fn(i32) -> bool {
        move |id| ids.contains(&id)
    }

    const OVERRIDES: &str = "
[[Override]]
SpEffect = [1111, 1112]
Player.Roll = 1.5

[[Override]]
SpEffect = 2222
Player.Attack = 1.6
Player.Roll = 2.0
Torrent.All = 2.5
";

    #[test]
    fn override_needs_any_of_its_sp_effects() {
        let config: Config = toml::from_str(OVERRIDES).unwrap();
        let (speeds, active) = config.effective_speed(has(&[1112]));
        assert_eq!(active, vec![0]);
        assert_eq!(speeds.player.roll, 1.5);
        assert_eq!(speeds.player.attack, 1.2); // [Player] default
        assert_eq!(speeds.torrent.all, 1.0);
    }

    #[test]
    fn active_overrides_stack_and_the_later_one_wins() {
        let config: Config = toml::from_str(OVERRIDES).unwrap();
        let (speeds, active) = config.effective_speed(has(&[1111, 2222]));
        assert_eq!(active, vec![0, 1]);
        assert_eq!(speeds.player.roll, 2.0);
        assert_eq!(speeds.player.attack, 1.6);
        assert_eq!(speeds.player.walk, 1.2);
        assert_eq!(speeds.torrent.all, 2.5);
    }

    #[test]
    fn no_active_override_keeps_speed() {
        let config: Config = toml::from_str(OVERRIDES).unwrap();
        let (speeds, active) = config.effective_speed(has(&[]));
        assert!(active.is_empty());
        assert_eq!(speeds.player, Player::default());
        assert_eq!(speeds.torrent, Torrent::default());
    }

    #[test]
    fn override_as_sub_tables_works_too() {
        let config: Config =
            toml::from_str("[[Override]]\nSpEffect = 1\n[Override.Player]\nRun = 3\n").unwrap();
        assert_eq!(config.overrides[0].player.run, Some(3.0));
    }

    #[test]
    fn sp_effect_ids_as_strings_are_accepted() {
        let config: Config =
            toml::from_str("[[Override]]\nSpEffect = [\"1111\", 2222]\nTorrent.All = 2\n").unwrap();
        assert_eq!(config.overrides[0].sp_effect, vec![1111, 2222]);
        assert_eq!(config.overrides[0].torrent.all, Some(2.0));
    }

    #[test]
    fn override_errors_are_reported() {
        for (text, needle) in [
            ("[[Override]]\nPlayer.Roll = 1.5\n", "SpEffect"),
            ("[[Override]]\nSpEffect = []\n", "empty"),
            ("[[Override]]\nSpEffect = \"abc\"\n", "not a number"),
            ("[[Override]]\nSpEffect = 1\nPlayer.Rol = 1\n", "Rol"),
            ("[[Override]]\nSpEffect = 1\nRoll = 1\n", "Roll"),
        ] {
            let err = toml::from_str::<Config>(text).unwrap_err().to_string();
            assert!(err.contains(needle), "{text:?} -> {err}");
        }
    }

    #[test]
    fn integer_value_is_accepted_as_a_speed() {
        // Users will write `All = 2`, not `2.0`.
        let parsed: Config = toml::from_str("[Torrent]\nAll = 2\n").unwrap();
        assert_eq!(parsed.torrent.all, 2.0);
    }

    #[test]
    fn steps_cover_every_version() {
        // The template's ConfigVersion is the current version: one step per
        // version after 1.
        let doc: toml::Value = toml::from_str(TEMPLATE).unwrap();
        let current = doc["General"]["ConfigVersion"].as_integer().unwrap();
        assert_eq!(STEPS.len() as i64, current - 1);
    }
}
