//! **G04.** Every relative documentation link resolves, inside the repository, to a file and an anchor that
//! exist, and the decision index lists every record.
//!
//! A README pointing at a document nobody wrote is the cheapest possible broken promise, and the one a
//! reader hits first. The decision index is the same failure in a place that matters more: a record written
//! and never listed is a decision nobody can find, which makes the whole set look complete when it is not.
//!
//! Links are read in every form Markdown and HTML give them: inline, reference-style, and raw `href` or
//! `src` attributes. A target that climbs out of the repository resolves on one machine and breaks on the
//! next, so it is refused even when the file happens to exist here.

use adiungere_guarantees::{TextFile, lines_with_fence_state, read_at, repository_root, tracked_text_files};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every link target in a document, with code blocks skipped, as `(line, target)`.
fn links_in(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();

    for (number, line, inside_fence) in lines_with_fence_state(source) {
        if inside_fence {
            continue;
        }

        found.extend(inline_links(line).map(|target| (number, target)));
        found.extend(reference_definition(line).map(|target| (number, target)));
        found.extend(html_attributes(line).map(|target| (number, target)));
    }

    found
}

/// `[text](target)` and `![alt](target)`.
fn inline_links(line: &str) -> impl Iterator<Item = String> + '_ {
    let mut rest = line;

    std::iter::from_fn(move || {
        let start = rest.find("](")?;
        let after = rest.get(start + 2..)?;
        let end = after.find(')')?;
        let target = after.get(..end)?.trim();
        rest = after.get(end + 1..).unwrap_or("");

        let target = target
            .split_once(' ')
            .map_or(target, |(url, _)| url)
            .trim_matches(|c| c == '<' || c == '>');

        Some(target.to_owned())
    })
}

/// `[label]: target` at the start of a line.
fn reference_definition(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix('[')?;
    let (label, after) = rest.split_once("]:")?;

    if label.is_empty() || label.starts_with('^') {
        return None;
    }

    let target = after.split_whitespace().next()?;

    Some(target.trim_matches(|c| c == '<' || c == '>').to_owned())
}

/// `href="…"` and `src="…"` in raw HTML.
fn html_attributes(line: &str) -> impl Iterator<Item = String> + '_ {
    ["href=\"", "src=\"", "href='", "src='"]
        .into_iter()
        .flat_map(move |attribute| {
            let quote = attribute.chars().last().unwrap_or('"');

            line.match_indices(attribute).filter_map(move |(start, _)| {
                let after = line.get(start + attribute.len()..)?;
                let end = after.find(quote)?;

                after.get(..end).map(str::to_owned)
            })
        })
}

fn is_relative(target: &str) -> bool {
    !target.is_empty()
        && !target.starts_with('#')
        && !target.contains("://")
        && !target.starts_with("mailto:")
        && !target.starts_with("tel:")
}

/// Where a relative target points, split into its path and its fragment.
fn split_target(target: &str) -> (&str, Option<&str>) {
    match target.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (target, None),
    }
}

fn resolved(document: &str, path: &str) -> PathBuf {
    let directory = Path::new(document).parent().unwrap_or(Path::new(""));

    repository_root().join(directory).join(path)
}

/// Whether a resolved path stays inside the repository once `..` is applied.
fn stays_inside(path: &Path) -> bool {
    let Ok(root) = repository_root().canonicalize() else {
        return false;
    };

    path.canonicalize()
        .is_ok_and(|canonical| canonical.starts_with(&root))
}

/// The anchors a Markdown document offers, as the renderer derives them from its headings.
fn anchors_of(source: &str) -> Vec<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut anchors = Vec::new();

    for (_, line, inside_fence) in lines_with_fence_state(source) {
        if inside_fence || !line.starts_with('#') {
            continue;
        }

        let text = line.trim_start_matches('#').trim();
        let base: String = text
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
            .map(|c| if c == ' ' { '-' } else { c })
            .collect();

        let count = seen.entry(base.clone()).or_insert(0);
        let anchor = if *count == 0 {
            base.clone()
        } else {
            format!("{base}-{count}")
        };
        *count += 1;
        anchors.push(anchor);
    }

    anchors
}

#[test]
fn every_relative_link_resolves_inside_the_repository() {
    // Arrange
    let documents: Vec<TextFile> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(TextFile::is_markdown)
        .collect();
    let mut broken = Vec::new();

    // Act
    for document in &documents {
        for (line, target) in links_in(&document.contents) {
            if !is_relative(&target) {
                continue;
            }

            let (path, fragment) = split_target(&target);
            let file = if path.is_empty() {
                repository_root().join(&document.path)
            } else {
                resolved(&document.path, path)
            };

            if !file.exists() {
                broken.push(format!("{}:{line} -> {target} (does not exist)", document.path));
                continue;
            }

            if !stays_inside(&file) {
                broken.push(format!(
                    "{}:{line} -> {target} (leaves the repository)",
                    document.path
                ));
                continue;
            }

            if let Some(fragment) = fragment
                && file.is_file()
                && adiungere_guarantees::is_markdown_path(&file.to_string_lossy())
            {
                let source = std::fs::read_to_string(&file).unwrap_or_default();

                if !anchors_of(&source).iter().any(|anchor| anchor == fragment) {
                    broken.push(format!("{}:{line} -> {target} (no such anchor)", document.path));
                }
            }
        }
    }

    // Assert
    assert!(
        broken.is_empty(),
        "These links point at something that does not exist or is not in the repository:\n  {}",
        broken.join("\n  ")
    );
}

#[test]
fn the_decision_index_links_every_record() {
    // Arrange
    let index = read_at("docs/adr/README.md").unwrap();
    let linked: Vec<String> = links_in(&index).into_iter().map(|(_, target)| target).collect();
    let records: Vec<String> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.path.starts_with("docs/adr/") && file.is_markdown())
        .map(|file| file.name().to_owned())
        .filter(|name| name != "README.md" && name != "0000-template.md")
        .collect();

    // Act
    let unlisted: Vec<&String> = records
        .iter()
        .filter(|name| !linked.iter().any(|target| target == *name))
        .collect();

    // Assert
    assert!(
        !records.is_empty(),
        "No decision record was found; this check is inert."
    );
    assert!(
        unlisted.is_empty(),
        "These decision records exist and the index does not link them:\n  {}",
        unlisted
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn every_form_of_link_is_extracted() {
    // Arrange
    let sample = "See [a](docs/a.md) and ![b](img/b.png \"title\").\n\
                  [ref]: docs/c.md\n\
                  <a href=\"docs/d.md\">d</a> <img src='img/e.png'>\n\
                  Not this: [^1]: footnote, nor `[x](in-code.md)`.\n\
                  ```\n[fenced](never.md)\n```\n";

    // Act
    let found: Vec<String> = links_in(sample).into_iter().map(|(_, t)| t).collect();

    // Assert
    assert_eq!(
        found,
        [
            "docs/a.md",
            "img/b.png",
            "docs/c.md",
            "docs/d.md",
            "img/e.png",
            "in-code.md"
        ]
    );
}

#[test]
fn a_link_that_climbs_out_of_the_repository_is_refused() {
    // Act
    let outside = resolved("README.md", "../README.md");

    // Assert
    assert!(
        !stays_inside(&outside),
        "A target above the root must be refused even if it exists."
    );
    assert!(stays_inside(&resolved("docs/roadmap.md", "../README.md")));
}

#[test]
fn anchors_follow_the_renderer_and_a_missing_one_is_detected() {
    // Arrange
    let source =
        "# Guarantees\n\n## Enforced today\n\n## Enforced today\n\n## What it proves, and what it does not\n";

    // Act
    let anchors = anchors_of(source);

    // Assert
    assert_eq!(
        anchors,
        [
            "guarantees",
            "enforced-today",
            "enforced-today-1",
            "what-it-proves-and-what-it-does-not"
        ]
    );
    assert!(!anchors.iter().any(|anchor| anchor == "nothing-here"));
}

#[test]
fn the_scan_reads_real_documents_and_real_links() {
    // Act
    let documents: Vec<TextFile> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(TextFile::is_markdown)
        .collect();
    let links: usize = documents.iter().map(|file| links_in(&file.contents).len()).sum();

    // Assert
    assert!(
        documents.len() > 20,
        "Only {} documents were read; the checks above are inert.",
        documents.len()
    );
    assert!(
        links > 50,
        "Only {links} links were extracted; the checks above are nearly inert."
    );
}
