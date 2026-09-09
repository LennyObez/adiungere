//! **G04.** Every relative documentation link resolves, and the decision index lists every record.
//!
//! A README pointing at a document nobody wrote is the cheapest possible broken promise, and the one a
//! reader hits first. The decision index is the same failure in a place that matters more: a record written
//! and never listed is a decision nobody can find, which makes the whole set look complete when it is not.

use adiungere_guarantees::{lines_with_fence_state, read_at, repository_root, tracked_text_files};
use std::path::Path;

/// Every relative link target in a document, with code blocks skipped.
fn links_in(source: &str) -> Vec<String> {
    let mut found = Vec::new();

    for (_, line, inside_fence) in lines_with_fence_state(source) {
        if inside_fence {
            continue;
        }

        let mut rest = line;

        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];

            let Some(end) = after.find(')') else { break };

            found.push(after[..end].to_owned());
            rest = &after[end + 1..];
        }
    }

    found
}

fn is_relative(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('#')
        && !target.starts_with("http://")
        && !target.starts_with("https://")
        && !target.starts_with("mailto:")
}

fn resolved(document: &str, target: &str) -> std::path::PathBuf {
    let without_fragment = target.split('#').next().unwrap_or(target);
    let directory = Path::new(document).parent().unwrap_or(Path::new(""));

    repository_root().join(directory).join(without_fragment)
}

#[test]
fn every_relative_link_resolves() {
    // Arrange
    let documents: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.has_extension("md"))
        .collect();
    let mut broken = Vec::new();

    // Act
    for document in &documents {
        for target in links_in(&document.contents) {
            if is_relative(&target) && !resolved(&document.path, &target).exists() {
                broken.push(format!("{} -> {target}", document.path));
            }
        }
    }

    // Assert
    assert!(
        broken.is_empty(),
        "These links point at paths that do not exist:\n  {}",
        broken.join("\n  ")
    );
}

#[test]
fn the_decision_index_lists_every_record() {
    // Arrange
    let index = read_at("docs/adr/README.md").unwrap();
    let records: Vec<String> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.path.starts_with("docs/adr/") && file.has_extension("md"))
        .map(|file| file.path)
        .filter(|path| !path.ends_with("/README.md") && !path.ends_with("0000-template.md"))
        .collect();

    // Act
    let unlisted: Vec<&String> = records
        .iter()
        .filter(|path| {
            let name = path.rsplit('/').next().unwrap_or(path);
            !index.contains(name)
        })
        .collect();

    // Assert
    assert!(
        !records.is_empty(),
        "No decision record was found; this check is inert."
    );
    assert!(
        unlisted.is_empty(),
        "These decision records exist and the index does not list them:\n  {}",
        unlisted
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn the_extraction_finds_a_link_and_ignores_an_absolute_one() {
    // Arrange
    let sample = "See [the roadmap](docs/roadmap.md) and [the site](https://adiungere.com/).";

    // Act
    let found = links_in(sample);
    let relative: Vec<&String> = found.iter().filter(|target| is_relative(target)).collect();

    // Assert
    assert_eq!(found.len(), 2, "Both links should have been extracted: {found:?}");
    assert_eq!(
        relative.len(),
        1,
        "Only the relative link should be checked: {relative:?}"
    );
    assert_eq!(
        relative.first().map(|target| target.as_str()),
        Some("docs/roadmap.md")
    );
}

#[test]
fn a_link_inside_a_code_block_is_not_followed() {
    // Documents show example markup. Following a link from an example would demand that the example's
    // imaginary file exist.

    // Arrange
    let sample = "Real: [here](README.md)\n\n```markdown\n[an example](does-not-exist.md)\n```\n";

    // Act
    let found = links_in(sample);

    // Assert
    assert_eq!(found, vec!["README.md".to_owned()]);
}

#[test]
fn the_check_detects_a_link_to_nothing() {
    // Arrange
    let target = "docs/a-document-nobody-wrote.md";

    // Act
    let exists = resolved("README.md", target).exists();

    // Assert
    assert!(!exists, "A link to a missing file must be detected as missing.");
}

#[test]
fn the_scan_reads_real_documents() {
    // Act
    let documents: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.has_extension("md"))
        .collect();
    let links: usize = documents.iter().map(|file| links_in(&file.contents).len()).sum();

    // Assert
    assert!(
        !documents.is_empty(),
        "No document was read; the checks above are inert."
    );
    assert!(links > 0, "No link was extracted; the checks above are inert.");
}
