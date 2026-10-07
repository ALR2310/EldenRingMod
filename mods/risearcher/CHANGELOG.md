# RiseArcher - Changelog

Release notes shown to users on the Nexus changelog tab: one short bullet per
user-facing change, plain text. Add new items under `[Unreleased]`; the
`deploy-nexus-mod` skill turns that section into a version when publishing.

## [Unreleased]

## [2.0.0]

- Rewritten as a Rust DLL - patches EquipParamWeapon/Bullet live in memory via fromsoftware-rs instead of shipping a modified regulation.bin, so it no longer conflicts with other regulation.bin mods.
- Every multiplier/value is now configurable via RiseArcher.ini instead of fixed at build time.

## [1.16.1]

- Static regulation.bin edit via Smithbox Mass Edit - bow/crossbow/ballista/arrow/bolt overhaul.
