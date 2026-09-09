//! **G03.** Every directory holding source is covered by a workflow path filter.
//!
//! One repository holding six surfaces is only safe if no directory can be added that no pipeline builds. A
//! path filter that is too narrow does not fail; it silently skips a build, and the pull request is green.
//!
//! The directories are discovered from what git tracks, never from a list written here. A hard-coded list
//! cannot detect the very thing this check exists to catch, which is a directory nobody remembered.
//!
//! A directory needs a pipeline only once it holds something to build. Documentation-only directories are
//! covered by the workflow that carries no path filter at all; the moment source lands in one of them, this
//! check starts demanding a pipeline.

use adiungere_guarantees::{TextFile, tracked_paths, workflow_files};
use std::collections::BTreeSet;

/// Every quoted string in a workflow that looks like a path filter.
fn declared_filters(workflows: &[TextFile]) -> BTreeSet<String> {
    let mut filters = BTreeSet::new();

    for file in workflows {
        for line in file.contents.lines() {
            let trimmed = line.trim();

            let Some(candidate) = trimmed
                .strip_prefix("- '")
                .and_then(|rest| rest.strip_suffix('\''))
            else {
                continue;
            };

            if candidate.contains('/') {
                filters.insert(candidate.to_owned());
            }
        }
    }

    filters
}

/// Whether a path is prose rather than something a pipeline has to build.
fn is_markdown(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

/// Directories at the first and second level that hold at least one tracked file that is not Markdown.
fn directories_holding_source(paths: &[String]) -> BTreeSet<String> {
    let mut directories = BTreeSet::new();

    for path in paths {
        if is_markdown(path) {
            continue;
        }

        let segments: Vec<&str> = path.split('/').collect();

        if segments.len() > 1
            && let Some(first) = segments.first()
        {
            directories.insert((*first).to_owned());

            if segments.len() > 2
                && let Some(second) = segments.get(1)
            {
                directories.insert(format!("{first}/{second}"));
            }
        }
    }

    directories
}

fn is_covered(directory: &str, filters: &BTreeSet<String>) -> bool {
    let mut probe = Some(directory);

    while let Some(current) = probe {
        if filters
            .iter()
            .any(|filter| filter.starts_with(&format!("{current}/")))
        {
            return true;
        }

        probe = current.rsplit_once('/').map(|(parent, _)| parent);
    }

    false
}

#[test]
fn every_directory_holding_source_is_built_by_a_pipeline() {
    // Arrange
    let workflows = workflow_files().unwrap();
    let paths = tracked_paths().unwrap();
    let filters = declared_filters(&workflows);
    let directories = directories_holding_source(&paths);

    // Act
    let uncovered: Vec<&String> = directories
        .iter()
        .filter(|directory| !is_covered(directory, &filters))
        .collect();

    // Assert
    assert!(
        uncovered.is_empty(),
        "These directories hold source and no workflow path filter builds them. Add a pipeline, or extend \
         an existing one:\n  {}",
        uncovered
            .iter()
            .map(|directory| directory.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn the_check_detects_a_directory_nobody_built() {
    // A check that considered everything covered would pass forever while proving nothing.

    // Arrange
    let workflows = workflow_files().unwrap();
    let filters = declared_filters(&workflows);

    // Act
    let verdict = is_covered("a-directory-nobody-declared", &filters);

    // Assert
    assert!(
        !verdict,
        "A directory named by no filter must be reported as uncovered."
    );
}

#[test]
fn the_check_accepts_a_directory_an_ancestor_filter_covers() {
    // A filter may legitimately name a parent, so coverage climbs. Without this the check would demand a
    // filter per level and be worked around rather than satisfied.

    // Arrange
    let filters: BTreeSet<String> = ["apps/site/**".to_owned()].into_iter().collect();

    // Act
    let covered_directly = is_covered("apps/site", &filters);
    let covered_by_ancestor = is_covered("apps", &filters);

    // Assert
    assert!(
        covered_directly,
        "A directory its own filter names must be covered."
    );
    assert!(
        covered_by_ancestor,
        "A parent of a filtered directory must be covered."
    );
}

#[test]
fn a_documentation_only_directory_is_not_demanded_of() {
    // The complement. Demanding a pipeline for a directory holding only prose would make the guarantee
    // unsatisfiable, and the usual answer to that is a filter that builds nothing.

    // Arrange
    let paths = vec![
        "docs/roadmap.md".to_owned(),
        "docs/adr/0001-a-single-repository.md".to_owned(),
    ];

    // Act
    let directories = directories_holding_source(&paths);

    // Assert
    assert!(
        directories.is_empty(),
        "Prose needs no build pipeline: {directories:?}"
    );
}

#[test]
fn the_scan_reads_real_directories_and_real_filters() {
    // Act
    let workflows = workflow_files().unwrap();
    let paths = tracked_paths().unwrap();
    let filters = declared_filters(&workflows);
    let directories = directories_holding_source(&paths);

    // Assert
    assert!(
        !filters.is_empty(),
        "No path filter was read; the check above is inert."
    );
    assert!(
        !directories.is_empty(),
        "No directory was discovered; the check above is inert."
    );
    assert!(
        directories.contains("core"),
        "The Rust workspace was not discovered, so the scan is not reading the repository."
    );
}
