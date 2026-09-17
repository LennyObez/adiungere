//! **G18.** Comparing a manifest with a file finds exactly what changed: one flipped byte in a sample is
//! that track's finding and the whole file's, and nothing else; one flipped byte in the vendor box is that
//! box's finding and the whole file's, and nothing else.
//!
//! A comparison that reported everything as different when one byte changed would tell a person nothing
//! about what happened to their recording, and a comparison that missed a flipped byte in the vendor box
//! would be silent about the very material this product exists to preserve.

use adiungere_fixtures::{Spec, build};
use adiungere_isobmff::SliceSource;
use adiungere_manifest::{Manifest, Outcome, Producer, Scope, Subject, VendorLocation, verify};

fn manifest_of(bytes: &[u8]) -> Result<Manifest, adiungere_manifest::Error> {
    let mut source = SliceSource::new(bytes);
    adiungere_manifest::build(
        &mut source,
        "clip.mp4",
        Scope::Full,
        &Producer::current("test", "0"),
        None,
    )
}

/// The subjects whose outcome is `Differs`, in order.
fn differing(bytes: &[u8], manifest: &Manifest) -> Result<Vec<Subject>, adiungere_manifest::Error> {
    let mut source = SliceSource::new(bytes);
    let verification = verify(manifest, &mut source)?;
    Ok(verification
        .findings
        .into_iter()
        .filter(|finding| matches!(finding.outcome, Outcome::Differs { .. }))
        .map(|finding| finding.subject)
        .collect())
}

#[test]
fn a_flipped_byte_in_the_rear_track_is_that_track_and_the_whole_file() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    let manifest = manifest_of(&bytes).unwrap();
    let front_first_chunk: usize = fixture.expected.tracks[0]
        .samples
        .iter()
        .take(15)
        .map(Vec::len)
        .sum();
    let rear_first_sample = usize::try_from(fixture.expected.mdat_offset).unwrap() + 8 + front_first_chunk;
    bytes[rear_first_sample + 2] ^= 0x40;

    // Act
    let changed = differing(&bytes, &manifest).unwrap();

    // Assert
    assert_eq!(changed, vec![Subject::WholeFile, Subject::Track { index: 1 }]);
}

#[test]
fn a_flipped_byte_in_the_vendor_box_is_that_box_and_the_whole_file() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    let manifest = manifest_of(&bytes).unwrap();
    let vendor = manifest
        .vendor
        .boxes
        .iter()
        .find(|vendor| vendor.location == VendorLocation::UserData)
        .unwrap();
    bytes[usize::try_from(vendor.offset).unwrap() + 4096] ^= 0x01;

    // Act
    let changed = differing(&bytes, &manifest).unwrap();

    // Assert
    assert_eq!(
        changed,
        vec![
            Subject::WholeFile,
            Subject::VendorBox {
                location: VendorLocation::UserData,
                kind: "zvnd".to_owned()
            }
        ]
    );
}

#[test]
fn a_flipped_byte_in_the_top_level_vendor_box_is_that_box_and_the_whole_file() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    let manifest = manifest_of(&bytes).unwrap();
    let vendor = manifest
        .vendor
        .boxes
        .iter()
        .find(|vendor| vendor.location == VendorLocation::TopLevel)
        .unwrap();
    bytes[usize::try_from(vendor.offset).unwrap() + 9] ^= 0x01;

    // Act
    let changed = differing(&bytes, &manifest).unwrap();

    // Assert
    assert_eq!(
        changed,
        vec![
            Subject::WholeFile,
            Subject::VendorBox {
                location: VendorLocation::TopLevel,
                kind: "zzzz".to_owned()
            }
        ]
    );
}

#[test]
fn an_unchanged_file_has_no_differing_subject() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let manifest = manifest_of(&bytes).unwrap();

    // Act
    let changed = differing(&bytes, &manifest).unwrap();

    // Assert
    assert!(changed.is_empty());
}
