//! **G26.** The derivative detector raises every sign it knows on the synthetic corpus, and the command line
//! shows each one in its own words.
//!
//! A gallery hands out re-exports and transcodes as if they were the recording, and the product's answer
//! is a banner that says why this file is not what the camera wrote. A sign the detector defines but
//! nothing ever raises is a banner nobody has seen; a sign raised but never worded is a banner nobody can
//! read. So the corpus is scanned as one population beside the reference-like recording, every variant of
//! the sign type as its source declares it has to be raised by at least one file, the reference-like file
//! has to raise none, and the command line's description has to carry each sign's phrase from the wording
//! catalogue. A file whose name fits no grammar is the neutral case: it is described as such, never as
//! clean.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use adiungere_cli::media::{self, Output};
use adiungere_fixtures::{Spec, build, corpus};
use adiungere_guarantees::{read_at, rust_code_only};
use adiungere_scan::{Origin, Recording, scan};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-g26-{name}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        );
        let path = std::env::temp_dir().join(unique);
        let _ = std::fs::create_dir_all(&path);
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The name of the reference-like file, and the name every other corpus recording takes beside it: the
/// same grammar, a later minute, so the scanner reads them as one population.
fn recorder_name(minute: usize) -> String {
    format!("20260604_12{:02}23E.MP4", 23 + minute)
}

/// Writes the corpus into a directory as one population, and returns the name each recording took.
fn write_population(into: &Path) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut names = Vec::new();
    for (position, spec) in corpus()
        .into_iter()
        .filter(|spec| !spec.layout.sparse)
        .enumerate()
    {
        let name = recorder_name(position);
        build(&spec)?.write_to(&into.join(&name))?;
        names.push((spec.name.to_owned(), name));
    }
    Ok(names)
}

/// The origin of every file the scan assessed on its own, by name.
fn origins_of(recordings: &[Recording]) -> Vec<(String, Origin)> {
    recordings
        .iter()
        .filter_map(|recording| match recording {
            Recording::SingleFile { file, origin, .. }
            | Recording::Unpaired { file, origin, .. }
            | Recording::Unrecognised { file, origin, .. } => Some((file.probe.name.clone(), origin.clone())),
            Recording::Paired { .. } | Recording::Unreadable { .. } => None,
        })
        .collect()
}

/// The variant names of the sign type, read from the source that declares it.
fn declared_signs() -> Vec<String> {
    let source = rust_code_only(&read_at("core/crates/adiungere-scan/src/derivative.rs").unwrap_or_default());
    let mut inside = false;
    let mut names = Vec::new();
    for line in source.lines() {
        if line.starts_with("pub enum Signal") {
            inside = true;
            continue;
        }
        if inside && line.starts_with('}') {
            break;
        }
        if inside && line.starts_with("    ") && !line.starts_with("     ") {
            let name: String = line
                .trim()
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    names
}

/// The variant name a sign prints under, which is its debug form up to the first brace or space.
fn variant_of(sign: &adiungere_scan::Signal) -> String {
    format!("{sign:?}")
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect()
}

#[test]
fn every_sign_the_detector_declares_is_raised_by_the_corpus_and_none_by_the_reference() {
    // Arrange
    let directory = Temporary::new("population");
    let written = write_population(&directory.0).unwrap();
    let declared: BTreeSet<String> = declared_signs().into_iter().collect();

    // Act
    let recordings = scan(std::slice::from_ref(&directory.0), None).recordings;
    let origins = origins_of(&recordings);
    let raised: BTreeSet<String> = origins
        .iter()
        .flat_map(|(_, origin)| origin.signals.iter().map(variant_of))
        .collect();
    let reference = origins
        .iter()
        .find(|(name, _)| *name == recorder_name(0))
        .map(|(_, origin)| origin.clone());

    // Assert
    assert!(
        declared.len() >= 5,
        "only {} signs were read from the source; this check is inert",
        declared.len()
    );
    assert_eq!(
        origins.len(),
        written.len(),
        "every written file was scanned as a single file"
    );
    assert_eq!(
        raised, declared,
        "every sign the detector declares is raised by at least one corpus recording"
    );
    assert_eq!(
        reference.as_ref().map(|origin| origin.signals.len()),
        Some(0),
        "the reference-like recording raises no sign: {reference:?}"
    );
    assert_eq!(reference.map(|origin| origin.layout_known), Some(true));
}

#[test]
fn the_command_line_words_each_sign_from_the_catalogue() {
    // Arrange
    let directory = Temporary::new("wording");
    write_population(&directory.0).unwrap();
    let catalogue: serde_json::Value =
        serde_json::from_str(&read_at("core/crates/adiungere-manifest/wording/en.json").unwrap()).unwrap();
    let phrases: Vec<String> = catalogue["strings"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, _)| key.starts_with("structure.sign."))
        .filter_map(|(_, phrase)| phrase.as_str())
        .map(|phrase| phrase.split('{').next().unwrap_or_default().trim_end().to_owned())
        .collect();

    // Act
    let described = media::detect(std::slice::from_ref(&directory.0), Output::Text, None).unwrap();
    let missing: Vec<&String> = phrases
        .iter()
        .filter(|phrase| !described.text.contains(phrase.as_str()))
        .collect();

    // Assert
    assert!(
        phrases.len() >= 5,
        "only {} sign phrases were read from the catalogue; this check is inert",
        phrases.len()
    );
    assert!(
        missing.is_empty(),
        "these sign phrases never appear in the description:\n  {}\n\n{}",
        missing
            .iter()
            .map(|phrase| phrase.as_str())
            .collect::<Vec<_>>()
            .join("\n  "),
        described.text
    );
}

#[test]
fn a_file_outside_every_grammar_is_described_as_unplaced_and_never_as_clean() {
    // Arrange
    let directory = Temporary::new("unplaced");
    build(&Spec::reference_like())
        .unwrap()
        .write_to(&directory.0.join("holiday.mp4"))
        .unwrap();
    let catalogue: serde_json::Value =
        serde_json::from_str(&read_at("core/crates/adiungere-manifest/wording/en.json").unwrap()).unwrap();
    let unplaced = catalogue["strings"]["structure.unknown.layout"]
        .as_str()
        .unwrap()
        .to_owned();
    let clean = catalogue["strings"]["structure.rewrite.none"]
        .as_str()
        .unwrap()
        .to_owned();

    // Act
    let origins = origins_of(&scan(std::slice::from_ref(&directory.0), None).recordings);
    let described = media::detect(std::slice::from_ref(&directory.0), Output::Text, None).unwrap();

    // Assert
    assert_eq!(
        origins
            .iter()
            .map(|(_, origin)| (origin.layout_known, origin.signals.len()))
            .collect::<Vec<_>>(),
        vec![(false, 0)]
    );
    assert!(described.text.contains(&unplaced), "{}", described.text);
    assert!(!described.text.contains(&clean), "{}", described.text);
}

#[test]
fn a_rewritten_file_outside_every_grammar_still_shows_what_any_container_shows() {
    // A recording a gallery re-exported and renamed has lost its grammar, but not the signs that need no
    // camera to compare with: they are raised, and the signs that need a sibling are not.

    // Arrange
    let directory = Temporary::new("renamed");
    build(&Spec::reference_like())
        .unwrap()
        .write_to(&directory.0.join(recorder_name(0)))
        .unwrap();
    let rewritten = corpus()
        .into_iter()
        .find(|spec| spec.name == "rewritten")
        .unwrap();
    build(&rewritten)
        .unwrap()
        .write_to(&directory.0.join("VID_20260604_122323.mp4"))
        .unwrap();

    // Act
    let origins = origins_of(&scan(std::slice::from_ref(&directory.0), None).recordings);
    let renamed = origins
        .iter()
        .find(|(name, _)| name == "VID_20260604_122323.mp4")
        .map(|(_, origin)| origin.clone())
        .unwrap();

    // Assert
    assert!(!renamed.layout_known);
    assert_eq!(
        renamed.signals.iter().map(variant_of).collect::<Vec<_>>(),
        vec!["MoovAfterMdat", "ForeignMuxerTag"]
    );
}
