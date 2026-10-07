# AutoRegen - Changelog

Release notes shown to users on the Nexus changelog tab: one short bullet per
user-facing change, plain text. Add new items under `[Unreleased]`; the
`deploy-nexus-mod` skill turns that section into a version when publishing.

## [Unreleased]

## [2.6.3] - 2026-09-24

- Fixed install paths with non-ASCII characters.

## [2.6.2] - 2026-09-22

- Increased the gesture-active confirmation timeout.

## [2.6.1] - 2026-09-17

- Fixed sitting regen sometimes activating (or staying active) without the gesture actually being active.

## [2.6.0] - 2026-09-14

- Added an option to heal based on missing HP/FP/Stamina.
- Added an option to cap how far passive regen restores you.

## [2.5.1] - 2026-09-12

- Fixed a crash with Regen Per Hit enabled at Volcano Manor.
- Fixed occasional stuttering.

## [2.5.0] - 2026-09-10

- Added idle and sitting (via gesture) conditions for Regen Per Tick.
- Shows an in-game notification when the config is reloaded.

## [2.4.0] - 2026-09-09

- Updated for ER 1.17.1.

## [2.3.0] - 2026-09-02

- Added an option to exclude Ash of War hits from on-hit regen.
- Fixed backstabs losing their animation when on-hit regen was enabled.

## [2.2.0] - 2026-08-31

- Updated for Elden Ring 1.17.

## [2.1.1] - 2026-08-26

- Fixed a mod-loading issue.

## [2.1.0] - 2026-08-25

- On-hit regen: added options for ranged or both.

## [2.0.0] - 2026-08-18

- Added Stamina restore on hit.
- Added restore based on damage dealt, instead of a fixed amount.
- Added an in-combat / out-of-combat condition for healing.
- Added a quick reload hotkey (F5).
- Updated the config file.
- Rewritten from C++ to Rust.

## [1.1.1]

- Projectile damage no longer triggers on-hit healing.

## [1.1.0]

- Added restore-on-hit: HpOnHit / FpOnHit / HpPercentOnHit / FpPercentOnHit.
- Interval is now clamped to a 50ms floor.

## [1.0.0]

- HP/FP/Stamina regeneration over time.
