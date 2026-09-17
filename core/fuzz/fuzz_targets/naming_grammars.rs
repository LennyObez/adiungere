//! Arbitrary text into the naming grammars.

#![no_main]

use adiungere_scan::parse_name;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(name) = std::str::from_utf8(data) {
        let _ = parse_name(name);
    }
});
