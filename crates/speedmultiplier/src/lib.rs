#![allow(non_snake_case)] // crate name is "SpeedMultiplier" to control the output DLL's filename

mod config;
mod probe;
mod speed;

use common::{dll_dir, logger};

// The in-game config menu (`common::menu_schema`): this mod's tab.
common::export_menu_api! {
    tab: "Speed Multiplier",
    config: config::Config,
    path: config::path,
    reload: reload_config,
}

/// Reload from the hotkey or the menu: the config, then the log switch.
fn reload_config() -> Result<(), String> {
    config::reload()?;
    logger::set_enabled(config::get().logging.log_file);
    Ok(())
}

/// # Safety
/// This is exposed this way so the library loader can call it. Do not call it
/// yourself.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DllMain(hmodule: u64, reason: u32) -> bool {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason != DLL_PROCESS_ATTACH {
        return true;
    }

    std::thread::spawn(move || {
        let dir = dll_dir(hmodule);
        let report = config::init(&format!("{dir}\\SpeedMultiplier.toml"));

        // [Logging] LogFile gates the log file entirely - see common::logger.
        logger::set_enabled(config::get().logging.log_file);
        logger::init(&dir, "SpeedMultiplier.log");
        logger::install_panic_hook();
        logger::log("Activating SpeedMultiplier...");
        if report.created {
            logger::log("SpeedMultiplier.toml created with the default settings.");
        }
        if report.added_keys > 0 {
            logger::log(&format!(
                "SpeedMultiplier.toml updated: added {} new key(s) from a newer default template.",
                report.added_keys
            ));
        }
        if let Some((from, to)) = report.migrated {
            logger::log(&format!("SpeedMultiplier.toml updated from format version {from} to {to}."));
        }
        if let Some((file, current)) = report.newer {
            logger::warn(&format!(
                "SpeedMultiplier.toml is from a newer version of the mod (format {file}, this one knows {current}) - left as it is."
            ));
        }
        if !report.unknown.is_empty() {
            logger::warn(&format!(
                "SpeedMultiplier.toml: {} key(s) this version doesn't use were moved to comments at the end of the file:\n  {}",
                report.unknown.len(),
                report.unknown.join("\n  ")
            ));
        }
        if let Some(err) = report.error {
            logger::error(&format!(
                "SpeedMultiplier.toml has an error, running on the default settings:\n{err}"
            ));
        }

        std::thread::spawn(|| {
            common::reload::run_with(
                || config::get().general.reload_key.clone(),
                || config::get().general.reload_banner,
                reload_config,
            )
        });
        std::thread::spawn(speed::run);

        common::diag::log_environment_when_game_ready();
        probe::run();
    });

    true
}
