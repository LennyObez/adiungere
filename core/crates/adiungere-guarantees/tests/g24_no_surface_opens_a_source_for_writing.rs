//! **G24.** No code path opens a recording for writing, and no command changes a byte or the modification
//! time of the files it is given.
//!
//! An original is evidence. A tool that could write to it, even by accident, could not be trusted with it.
//! Two things hold the line: the library crates that read recordings contain no call that opens a file for
//! writing, the one exception being the scan cache, which writes its own file and never a recording; and
//! every command of the product, run on a recording, leaves that recording's bytes and modification time
//! exactly as they were.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use adiungere_cli::media::{self, MediaFailure, Output, Rendered, Selection};
use adiungere_cli::provenance;
use adiungere_fingerprint::Digest;
use adiungere_fixtures::{Spec, build};
use adiungere_guarantees::{rust_code_only, tracked_text_files_under};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-g24-{name}-{}-{:?}",
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

/// The calls that open or change a file, assembled from fragments so this file does not contain them.
fn writing_calls() -> Vec<String> {
    vec![
        format!("File::{}", "create"),
        format!("{}::new()", "OpenOptions"),
        format!(".{}(true)", "write"),
        format!(".{}(true)", "append"),
        format!("fs::{}(", "write"),
        format!("fs::{}(", "rename"),
        format!("fs::{}(", "remove_file"),
        format!("fs::{}(", "copy"),
        format!("set_{}(", "modified"),
        format!("set_{}(", "len"),
    ]
}

/// The files of the reading crates that may write, each to its own output and never to a recording.
const MAY_WRITE: [&str; 1] = ["core/crates/adiungere-scan/src/cache.rs"];

#[test]
fn the_reading_crates_never_open_a_file_for_writing() {
    // Arrange
    let files = tracked_text_files_under("core/crates/").unwrap();
    let sources: Vec<_> = files
        .iter()
        .filter(|file| file.has_extension("rs"))
        .filter(|file| file.path.contains("/src/"))
        .filter(|file| {
            [
                "adiungere-isobmff",
                "adiungere-fingerprint",
                "adiungere-manifest",
                "adiungere-scan",
            ]
            .iter()
            .any(|crate_name| file.path.contains(crate_name))
        })
        .collect();
    let calls = writing_calls();

    // Act
    let offending: Vec<String> = sources
        .iter()
        .filter(|file| !MAY_WRITE.contains(&file.path.as_str()))
        .flat_map(|file| {
            let code = rust_code_only(&file.contents);
            calls
                .iter()
                .filter(|call| code.contains(call.as_str()))
                .map(move |call| format!("{}: {call}", file.path))
                .collect::<Vec<_>>()
        })
        .collect();

    // Assert
    assert!(
        sources.len() >= 15,
        "only {} sources were read; this check is inert",
        sources.len()
    );
    assert!(
        offending.is_empty(),
        "These files of the reading crates open or change a file:\n  {}",
        offending.join("\n  ")
    );
}

#[test]
fn the_exempted_file_writes_only_its_own_cache() {
    // Arrange
    let files = tracked_text_files_under("core/crates/adiungere-scan/src/").unwrap();
    let cache = files.iter().find(|file| file.path == MAY_WRITE[0]).unwrap();

    // Act
    let code = rust_code_only(&cache.contents);
    let writes = code.matches(&format!("fs::{}(", "write")).count();
    let opens = code.matches(&format!("File::{}", "create")).count();

    // Assert
    assert_eq!(writes, 1, "the cache writes itself, once");
    assert_eq!(opens, 0);
}

/// One command of the product, run against the recording under test.
type Step<'a> = (&'a str, Box<dyn Fn() -> Result<Rendered, MediaFailure> + 'a>);

/// The digest of a file's bytes and its modification time, which a read-only tool leaves untouched.
fn state_of(path: &Path) -> (String, std::time::SystemTime) {
    let bytes = std::fs::read(path).unwrap_or_default();
    let modified = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    (Digest::of(&bytes).to_string(), modified)
}

#[test]
fn every_command_leaves_the_recording_it_is_given_exactly_as_it_was() {
    // Arrange
    let directory = Temporary::new("commands");
    let clip = directory.0.join("20260604_122323E.MP4");
    build(&Spec::reference_like()).unwrap().write_to(&clip).unwrap();
    let stamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    std::fs::File::options()
        .write(true)
        .open(&clip)
        .unwrap()
        .set_modified(stamp)
        .unwrap();
    let before = state_of(&clip);
    let manifest = directory.0.join("clip.manifest.json");
    let out = directory.0.join("out.mp4");
    let cache = directory.0.join("cache.json");
    let signed = directory.0.join("signed.mp4");
    let stop = AtomicBool::new(false);
    let files = [clip.clone()];
    let pair = [clip.clone(), clip.clone()];
    let signing = provenance::Signing {
        credentialling: provenance::Credentialling::Ephemeral,
        time_authority: None,
    };

    // Act and assert, one command at a time, so the one that wrote is named.
    let steps: Vec<Step<'_>> = vec![
        ("inspect", Box::new(|| media::inspect(&clip, Output::Text))),
        (
            "fingerprint",
            Box::new(|| media::fingerprint(&clip, Output::Text, Some(&manifest))),
        ),
        (
            "detect",
            Box::new(|| media::detect(&files, Output::Text, Some(&cache))),
        ),
        (
            "verify",
            Box::new(|| media::verify(&manifest, &clip, Output::Text)),
        ),
        (
            "export rear",
            Box::new(|| media::export(&files, &Selection::Rear, &out, None, None, Output::Json, &stop)),
        ),
        (
            "export join",
            Box::new(|| media::export(&pair, &Selection::Both, &out, None, None, Output::Json, &stop)),
        ),
        (
            "export signed",
            Box::new(|| {
                media::export(
                    &files,
                    &Selection::Rear,
                    &signed,
                    None,
                    Some(&signing),
                    Output::Json,
                    &stop,
                )
            }),
        ),
        (
            "sign beside",
            Box::new(|| provenance::sign(&clip, &[], false, None, &signing, Output::Json)),
        ),
        (
            "verify with credentials",
            Box::new(|| media::verify(&manifest, &clip, Output::Text)),
        ),
    ];
    for (name, step) in steps {
        let outcome = step();
        assert!(outcome.is_ok(), "{name}: {:?}", outcome.err());
        assert_eq!(
            state_of(&clip),
            before,
            "{name} changed the recording or its modification time"
        );
    }
    assert!(before.1 == stamp, "the stamp was set and read back");
}

#[test]
fn an_export_asked_to_write_over_its_source_refuses_and_leaves_it_as_it_was() {
    // The one command that writes must not be talked into writing where it reads, by the output path,
    // by the partial file's path or by the manifest's path, under another spelling of the same file.

    // Arrange
    let directory = Temporary::new("overwrite");
    let clip = directory.0.join("20260604_122323E.MP4");
    build(&Spec::reference_like()).unwrap().write_to(&clip).unwrap();
    let before = state_of(&clip);
    let files = [clip.clone()];
    let stop = AtomicBool::new(false);
    let same_by_another_route = directory
        .0
        .join("..")
        .join(directory.0.file_name().unwrap_or_default())
        .join("20260604_122323E.MP4");
    let elsewhere = directory.0.join("out.mp4");

    // Act
    let as_output = media::export(
        &files,
        &Selection::Rear,
        &same_by_another_route,
        None,
        None,
        Output::Json,
        &stop,
    );
    let as_manifest = media::export(
        &files,
        &Selection::Rear,
        &elsewhere,
        Some(&same_by_another_route),
        None,
        Output::Json,
        &stop,
    );

    // Assert
    assert!(
        matches!(as_output, Err(MediaFailure::Overwrite { .. })),
        "{as_output:?}"
    );
    assert!(
        matches!(as_manifest, Err(MediaFailure::Overwrite { .. })),
        "{as_manifest:?}"
    );
    assert_eq!(state_of(&clip), before, "the recording was changed");
    assert!(!elsewhere.exists(), "nothing was written before the refusal");
}
