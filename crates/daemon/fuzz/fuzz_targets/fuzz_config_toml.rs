//! Fuzz du parsing de la configuration TOML du daemon.
//! Fuzzing of the daemon TOML configuration parsing.

#![no_main]

use libfuzzer_sys::fuzz_target;
use syswall_daemon::config::SysWallConfig;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = SysWallConfig::from_toml(s);
    }
});
