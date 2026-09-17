//! Arbitrary bytes into the reader, then every fingerprint over whatever it found.

#![no_main]

use adiungere_fingerprint::{annex_b_digest, file_digest, structural_fingerprint, track_fingerprint};
use adiungere_isobmff::{SliceSource, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut source = SliceSource::new(data);
    let Ok(container) = parse(&mut source) else {
        return;
    };
    let _ = structural_fingerprint(&container);
    let _ = file_digest(&mut source);
    let Ok(tracks) = container.tracks() else {
        return;
    };
    for track in &tracks {
        let _ = track_fingerprint(&mut source, &container, track);
        let _ = annex_b_digest(&mut source, &container, track);
    }
});
