#![allow(non_snake_case)] // crate name is "PassiveRunes" to control the output DLL's filename

mod rune;

use common::{config, dll_dir, logger};

// PassiveRunes.ini is embedded verbatim into the binary at compile time via
// include_str! - no resource compiler step needed. This is the single source
// of truth for the default config: edit PassiveRunes.ini, rebuild, done.
const DEFAULT_INI: &str = include_str!("../PassiveRunes.ini");

/// # Safety
/// This is exposed this way so the library loader can call it. Do not call it
/// yourself. Mirrors the entry point shape used across fromsoftware-rs's own
/// examples (e.g. examples/apply-speffect).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DllMain(hmodule: u64, reason: u32) -> bool {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason != DLL_PROCESS_ATTACH {
        return true;
    }

    std::thread::spawn(move || {
        let dir = dll_dir(hmodule);
        let ini_path = format!("{dir}\\PassiveRunes.ini");
        let migrated = config::load_or_create_default(&ini_path, DEFAULT_INI);

        // EnableLog gates the log file entirely (off by default) - same
        // convention as the original PassiveRunes' Logger::Init(dir, enabled).
        if config::get_bool("EnableLog", false) {
            logger::init(&dir, "PassiveRunes.log");
        }
        logger::log("Activating PassiveRunes...");
        if migrated > 0 {
            logger::log(&format!(
                "PassiveRunes.ini updated: added {migrated} new key(s) from a newer default template."
            ));
        }

        rune::run();
    });

    true
}
