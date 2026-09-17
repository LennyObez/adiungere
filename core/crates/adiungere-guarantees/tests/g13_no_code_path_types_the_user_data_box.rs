//! **G13.** No code path builds a typed user-data box, and an unknown child of the user-data box survives
//! every public entry point as its own bytes.
//!
//! The vendor telemetry lives under the user-data box. Every general-purpose library types that box and
//! drops the children it does not know, which is exactly the material this product exists to preserve. So
//! two things are refused: a type in any crate that models the user-data box with fields, and a public
//! operation after which the vendor child's bytes are no longer recoverable.

use adiungere_fingerprint::Digest;
use adiungere_fixtures::{Spec, build};
use adiungere_guarantees::{rust_code_only, tracked_text_files_under};
use adiungere_isobmff::{SliceSource, parse};
use adiungere_manifest::{Producer, Scope, VendorLocation};

/// Declarations that would mean a typed user-data box. Assembled from fragments so this file does not
/// contain what it forbids.
fn typed_declarations() -> Vec<String> {
    let names = ["Udta", "UserData", "UserDataBox"];
    let mut found = Vec::new();
    for name in names {
        found.push(format!("{} {name}", "struct"));
        found.push(format!("{} {name}", "enum"));
        found.push(format!("{} {name} ", "type"));
    }
    found
}

#[test]
fn no_crate_declares_a_typed_user_data_box() {
    // Arrange
    let files = tracked_text_files_under("core/crates/").unwrap();
    let sources: Vec<_> = files.iter().filter(|file| file.has_extension("rs")).collect();
    let declarations = typed_declarations();

    // Act
    let offending: Vec<String> = sources
        .iter()
        .flat_map(|file| {
            let code = rust_code_only(&file.contents);
            declarations
                .iter()
                .filter(|declaration| code.contains(declaration.as_str()))
                .map(move |declaration| format!("{}: {declaration}", file.path))
                .collect::<Vec<_>>()
        })
        .collect();

    // Assert
    assert!(
        sources.len() >= 20,
        "only {} sources were read; this check is inert",
        sources.len()
    );
    assert!(
        offending.is_empty(),
        "These files type the user-data box, which is how its vendor children get dropped:\n  {}",
        offending.join("\n  ")
    );
}

#[test]
fn the_vendor_child_is_recoverable_byte_for_byte_after_reading() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let expected = fixture.expected.vendor_box.clone().unwrap();
    let mut source = SliceSource::new(&bytes);

    // Act
    let container = parse(&mut source).unwrap();
    let children = container.udta_children();

    // Assert
    let vendor = children.first().unwrap();
    assert_eq!(container.bytes_of(vendor).unwrap(), expected.as_slice());
}

#[test]
fn the_manifest_records_the_vendor_child_by_the_digest_of_its_own_bytes() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let expected = Digest::of(fixture.expected.vendor_box.as_ref().unwrap());
    let mut source = SliceSource::new(&bytes);

    // Act
    let manifest = adiungere_manifest::build(
        &mut source,
        "clip.mp4",
        Scope::Structure,
        &Producer::current("test", "0"),
        None,
    )
    .unwrap();

    // Assert
    let recorded = manifest
        .vendor
        .boxes
        .iter()
        .find(|vendor| vendor.location == VendorLocation::UserData)
        .unwrap();
    assert_eq!(recorded.sha256, Some(expected));
    assert!(!manifest.vendor.interpreted);
}

#[test]
fn the_declaration_scan_matches_what_it_describes() {
    // Act
    let declarations = typed_declarations();
    let sample = format!("pub {} {}Box {{ children: Vec<Child> }}", "struct", "UserData");

    // Assert
    assert!(
        declarations
            .iter()
            .any(|declaration| sample.contains(declaration.as_str()))
    );
    assert!(
        !declarations
            .iter()
            .any(|declaration| "fn udta_children(&self)".contains(declaration.as_str()))
    );
}
