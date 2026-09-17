//! **G03.** Every directory holding source is built by a pipeline that a change to it triggers.
//!
//! One repository holding six surfaces is only safe if no directory can be added that no pipeline builds. A
//! path filter that is too narrow does not fail; it silently skips a build, and the pull request is green.
//!
//! The directories are discovered from what git tracks, never from a list written here. A hard-coded list
//! cannot detect the very thing this check exists to catch, which is a directory nobody remembered.
//!
//! Coverage is read from the `paths` list of a workflow's push trigger and from nothing else. A `paths-ignore`
//! list is the opposite of coverage, and a filter that names a sibling covers the sibling, never the parent.
//! One directory is exempt and named as such: `.github` is consumed by the platform rather than built by a
//! pipeline, and the workflows that carry no filter at all run over it on every change.

use adiungere_guarantees::{TextFile, is_markdown_path, tracked_paths, workflow_files};
use std::collections::BTreeSet;

/// The one directory whose contents are run by the platform rather than built by a pipeline.
const PLATFORM_DIRECTORY: &str = ".github";

/// Every entry under a `paths:` list in a workflow, in the order found.
fn declared_filters(workflows: &[TextFile]) -> BTreeSet<String> {
    let mut filters = BTreeSet::new();

    for file in workflows {
        let mut collecting = false;

        for line in file.contents.lines() {
            let trimmed = line.trim();

            if let Some(key) = trimmed.strip_suffix(':')
                && !key.contains(' ')
            {
                collecting = key == "paths";
                continue;
            }

            if !collecting {
                continue;
            }

            let Some(entry) = trimmed.strip_prefix("- ") else {
                collecting = false;
                continue;
            };

            let value = entry.trim().trim_matches(|c| c == '\'' || c == '"');

            if value.contains('/') {
                filters.insert(value.to_owned());
            }
        }
    }

    filters
}

/// Every directory that directly holds at least one tracked file that is not prose.
fn directories_holding_source(paths: &[String]) -> BTreeSet<String> {
    paths
        .iter()
        .filter(|path| !is_markdown_path(path))
        .filter_map(|path| path.rsplit_once('/').map(|(directory, _)| directory.to_owned()))
        .collect()
}

/// Whether a filter builds a directory: a glob over the directory or one of its ancestors, or a file named
/// directly inside the directory that is itself tracked source.
fn is_covered(directory: &str, filters: &BTreeSet<String>, paths: &[String]) -> bool {
    filters.iter().any(|filter| {
        if let Some(root) = filter.strip_suffix("/**") {
            return directory == root || directory.starts_with(&format!("{root}/"));
        }

        filter.rsplit_once('/').is_some_and(|(parent, _)| {
            parent == directory && !is_markdown_path(filter) && paths.iter().any(|path| path == filter)
        })
    })
}

fn is_platform(directory: &str) -> bool {
    directory == PLATFORM_DIRECTORY || directory.starts_with(&format!("{PLATFORM_DIRECTORY}/"))
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
        .filter(|directory| !is_platform(directory))
        .filter(|directory| !is_covered(directory, &filters, &paths))
        .collect();

    // Assert
    assert!(
        uncovered.is_empty(),
        "These directories hold source and no workflow path filter builds them. Add a pipeline, or extend \
         an existing one's paths list:\n  {}",
        uncovered
            .iter()
            .map(|directory| directory.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn a_sibling_filter_does_not_cover_the_parent() {
    // This is the hole that was found: a filter over one application directory read as coverage of every
    // application directory, so a new one could be added with no pipeline and the check stayed green.

    // Arrange
    let filters: BTreeSet<String> = ["apps/site/**".to_owned()].into_iter().collect();
    let paths = vec!["apps/site/public/index.html".to_owned()];

    // Act and assert
    assert!(is_covered("apps/site", &filters, &paths));
    assert!(is_covered("apps/site/public", &filters, &paths));
    assert!(
        !is_covered("apps", &filters, &paths),
        "The parent is not covered by a sibling's glob."
    );
    assert!(
        !is_covered("apps/windows", &filters, &paths),
        "A sibling is not covered by another's glob."
    );
}

#[test]
fn a_paths_ignore_list_is_not_coverage() {
    // Arrange
    let workflow = TextFile {
        path: ".github/workflows/x.yml".to_owned(),
        contents: "on:\n  push:\n    paths-ignore:\n      - 'core/**'\n    paths:\n      - 'apps/site/**'\n"
            .to_owned(),
    };

    // Act
    let filters = declared_filters(&[workflow]);

    // Assert
    assert_eq!(filters, ["apps/site/**".to_owned()].into_iter().collect());
}

#[test]
fn a_file_filter_covers_its_directory_only_when_it_names_tracked_source() {
    // Arrange
    let filters: BTreeSet<String> = ["core/README.md".to_owned(), "scripts/gate.sh".to_owned()]
        .into_iter()
        .collect();
    let paths = vec!["core/README.md".to_owned(), "scripts/gate.sh".to_owned()];

    // Act and assert
    assert!(
        !is_covered("core", &filters, &paths),
        "Prose named by a filter builds nothing."
    );
    assert!(
        is_covered("scripts", &filters, &paths),
        "A tracked source file named by a filter covers its directory."
    );
}

#[test]
fn a_documentation_only_directory_is_not_demanded_of() {
    // Arrange
    let paths = vec![
        "docs/roadmap.md".to_owned(),
        "docs/adr/0001-a-single-repository.md".to_owned(),
        "design/README.markdown".to_owned(),
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
        directories.contains("core/crates/adiungere-cli/src"),
        "The Rust sources were not discovered, so the scan is not reading the repository: {directories:?}"
    );
    assert!(
        directories.contains("scripts"),
        "The scripts directory was not discovered: {directories:?}"
    );
}
