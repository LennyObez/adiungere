//! Arbitrary bytes into the box reader. It may refuse; it may never panic.

#![no_main]

use adiungere_isobmff::{SliceSource, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut source = SliceSource::new(data);
    if let Ok(container) = parse(&mut source) {
        let _ = container.ftyp();
        let _ = container.mvhd();
        let _ = container.walk();
        let _ = container.ranges_tile_the_file();
        let _ = container.udta_children();
        let _ = container.unknown_top_level();
    }
});
