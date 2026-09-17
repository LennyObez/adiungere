//! Arbitrary bytes into the box reader, then every track's sample tables and sample iterator.

#![no_main]

use adiungere_isobmff::{SampleTable, SliceSource, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut source = SliceSource::new(data);
    let Ok(container) = parse(&mut source) else {
        return;
    };
    let Ok(tracks) = container.tracks() else {
        return;
    };
    for track in &tracks {
        let _ = track.duration_milliseconds();
        let _ = track.entry().and_then(|entry| entry.avc_configuration(&container));
        let _ = track.entry().and_then(|entry| entry.codec_string(&container));
        if let Ok(samples) = track.table.samples() {
            for sample in samples.iter().take(64) {
                let _ = SampleTable::read_sample(&mut source, sample);
            }
        }
    }
});
