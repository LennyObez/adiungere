//! Arbitrary text into the manifest reader, then the canonical form of whatever it accepted.

#![no_main]

use adiungere_manifest::Manifest;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(manifest) = Manifest::from_json(text) {
        let _ = manifest.to_canonical_json();
    }
});
