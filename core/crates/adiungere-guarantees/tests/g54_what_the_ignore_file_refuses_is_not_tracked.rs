//! **G54.** Nothing the ignore file refuses is tracked, and no tracked file exceeds the size cap.
//!
//! The ignore file says what never enters the repository: a recording, which carries satellite positions and
//! a route; signing material, which carries the ability to speak as this project; the working notes. It is
//! advisory. One forced add, one rename before the pattern was written, one path the pattern did not cover,
//! and the file is tracked while the ignore file still says it cannot be. The platform's push rules would
//! close that gap for a repository under an organisation; this repository is not one, so the suite closes it.
//!
//! The rules are read from `.gitignore` rather than written here, so the one place that says what is refused
//! stays the one place. The matcher is stricter than git in one respect, deliberately: it compares without
//! regard to case, because a recording called `CLIP.Mp4` is still a recording.
//!
//! The size cap exists for a different reason. A recording is large; source is not. A tracked file above the
//! cap is either a recording under a name the patterns do not know or a build artefact, and both are refused.

use adiungere_guarantees::{read_at, repository_root, tracked_paths};

/// The largest file the repository may track, in bytes.
const SIZE_CAP: u64 = 1_048_576;

/// One rule of the ignore file, reduced to what a tracked path can be checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Rule {
    /// `*.ext` or `name`: a file name pattern, matched against the last segment of any path.
    Name(String),
    /// `name/`: a directory name, matched against any segment of a path except the last.
    Directory(String),
    /// `/a/b/`: a directory at a fixed place, matched against the start of a path.
    AnchoredDirectory(String),
    /// `/a/b.txt` or `a/b.txt`: a file at a fixed place, matched against the whole path.
    AnchoredName(String),
}

/// The ignore file as rules, in order, with the negated ones separated out.
fn rules_in(ignore: &str) -> Result<(Vec<Rule>, Vec<Rule>), String> {
    let mut refused = Vec::new();
    let mut exempt = Vec::new();

    for (index, raw) in ignore.lines().enumerate() {
        let line = raw.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.contains('?') || line.contains('[') {
            return Err(format!(
                "line {}: this matcher understands `*` and nothing else; {line:?} would be matched wrongly \
                 in silence",
                index + 1
            ));
        }

        let (negated, pattern) = match line.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, line),
        };

        let rule = rule_of(pattern);

        if negated {
            exempt.push(rule);
        } else {
            refused.push(rule);
        }
    }

    Ok((refused, exempt))
}

/// Reads one pattern into a rule, following the ignore file's own conventions for slashes.
fn rule_of(pattern: &str) -> Rule {
    let pattern = pattern.strip_prefix("**/").unwrap_or(pattern);
    let (is_directory, pattern) = match pattern.strip_suffix('/') {
        Some(rest) => (true, rest),
        None => (false, pattern),
    };
    let pattern = pattern.to_lowercase();

    if let Some(anchored) = pattern.strip_prefix('/') {
        if is_directory {
            Rule::AnchoredDirectory(anchored.to_owned())
        } else {
            Rule::AnchoredName(anchored.to_owned())
        }
    } else if pattern.contains('/') {
        // A slash in the middle anchors the pattern to the file's own directory, which is the root here.
        if is_directory {
            Rule::AnchoredDirectory(pattern)
        } else {
            Rule::AnchoredName(pattern)
        }
    } else if is_directory {
        Rule::Directory(pattern)
    } else {
        Rule::Name(pattern)
    }
}

/// Whether `text` matches a pattern in which `*` stands for any run of characters, including none.
fn glob_matches(pattern: &str, text: &str) -> bool {
    let mut pieces = pattern.split('*');
    let Some(first) = pieces.next() else {
        return text.is_empty();
    };
    let Some(rest) = text.strip_prefix(first) else {
        return false;
    };
    let pieces: Vec<&str> = pieces.collect();
    let Some((last, middle)) = pieces.split_last() else {
        // No `*` at all: the whole text had to equal the pattern.
        return rest.is_empty();
    };

    let mut remaining = rest;

    for piece in middle {
        let Some(found) = remaining.find(piece) else {
            return false;
        };
        remaining = remaining.get(found + piece.len()..).unwrap_or("");
    }

    remaining.ends_with(last)
}

/// Whether a rule applies to a tracked path.
fn rule_matches(rule: &Rule, path: &str) -> bool {
    let lowered = path.to_lowercase();
    let name = lowered.rsplit('/').next().unwrap_or(&lowered);
    let segments: Vec<&str> = lowered.split('/').collect();
    let directories: &[&str] = segments.split_last().map_or(&[], |(_, rest)| rest);

    match rule {
        Rule::Name(pattern) => glob_matches(pattern, name),
        Rule::Directory(pattern) => directories.iter().any(|segment| glob_matches(pattern, segment)),
        Rule::AnchoredDirectory(prefix) => lowered.starts_with(&format!("{prefix}/")),
        Rule::AnchoredName(pattern) => glob_matches(pattern, &lowered),
    }
}

/// The first refusing rule a path falls under, unless an exemption lifts it.
fn refusal_for<'a>(path: &str, refused: &'a [Rule], exempt: &[Rule]) -> Option<&'a Rule> {
    if exempt.iter().any(|rule| rule_matches(rule, path)) {
        return None;
    }

    refused.iter().find(|rule| rule_matches(rule, path))
}

#[test]
fn nothing_the_ignore_file_refuses_is_tracked() {
    // Arrange
    let ignore = read_at(".gitignore").unwrap();
    let (refused, exempt) = rules_in(&ignore).unwrap();
    let paths = tracked_paths().unwrap();

    // Act
    let offending: Vec<String> = paths
        .iter()
        .filter_map(|path| refusal_for(path, &refused, &exempt).map(|rule| format!("{path} ({rule:?})")))
        .collect();

    // Assert
    assert!(
        refused.len() >= 20,
        "Only {} rules were read from the ignore file; this check is inert.",
        refused.len()
    );
    assert!(
        paths.len() >= 50,
        "Only {} tracked paths were listed; this check is inert.",
        paths.len()
    );
    assert!(
        offending.is_empty(),
        "These tracked files fall under a rule the ignore file states. The ignore file says they never enter \
         the repository, and they have. Remove them from the index and from every commit that carries \
         them:\n  {}",
        offending.join("\n  ")
    );
}

#[test]
fn no_tracked_file_exceeds_the_size_cap() {
    // Arrange
    let root = repository_root();
    let paths = tracked_paths().unwrap();

    // Act
    let oversized: Vec<String> = paths
        .iter()
        .filter_map(|path| {
            let size = std::fs::metadata(root.join(path)).ok()?.len();
            (size > SIZE_CAP).then(|| format!("{path} ({size} bytes)"))
        })
        .collect();

    // Assert
    assert!(
        paths.len() >= 50,
        "Only {} tracked paths were listed; this check is inert.",
        paths.len()
    );
    assert!(
        oversized.is_empty(),
        "These tracked files are larger than {SIZE_CAP} bytes. Source is never that large; a recording or a \
         build artefact is:\n  {}",
        oversized.join("\n  ")
    );
}

#[test]
fn the_matcher_reads_each_kind_of_rule_the_ignore_file_uses() {
    // The rules come from the real file, so the kinds present there are the kinds that have to be understood.
    // A kind this matcher had never seen would be read as something else and match nothing, in silence.

    // Arrange
    let ignore = read_at(".gitignore").unwrap();

    // Act
    let (refused, exempt) = rules_in(&ignore).unwrap();

    // Assert
    assert!(
        refused
            .iter()
            .any(|rule| matches!(rule, Rule::Name(pattern) if pattern.starts_with("*."))),
        "no extension rule was read"
    );
    assert!(
        refused
            .iter()
            .any(|rule| matches!(rule, Rule::Name(pattern) if !pattern.contains('*'))),
        "no bare file name rule was read"
    );
    assert!(
        refused
            .iter()
            .any(|rule| matches!(rule, Rule::AnchoredDirectory(_))),
        "no anchored directory rule was read"
    );
    assert!(
        refused.iter().any(|rule| matches!(rule, Rule::Directory(_))),
        "no directory name rule was read"
    );
    assert!(!exempt.is_empty(), "no exemption was read");
}

#[test]
fn the_matcher_matches_what_the_rules_describe() {
    // Arrange
    let (refused, exempt) = rules_in(
        "*.MP4\n*.key\nid_rsa\n!**/*.pub\n/core/fixtures/private/\n.direnv/\n**/google-services.json\n\
         .env.*\n!.env.example\n",
    )
    .unwrap();

    // Act
    let verdicts: Vec<(&str, bool)> = [
        "clips/CLIP.mp4",
        "clips/clip.Mp4",
        "apps/android/release.key",
        ".ssh/id_rsa",
        "core/fixtures/private/anything.bin",
        "tools/.direnv/cache",
        "apps/android/app/google-services.json",
        "apps/site/.env.production",
        "apps/site/.env.example",
        "keys/signing.pub",
        "core/fixtures/README.md",
        "docs/private-notes.md",
    ]
    .into_iter()
    .map(|path| (path, refusal_for(path, &refused, &exempt).is_some()))
    .collect();

    // Assert
    assert_eq!(
        verdicts,
        vec![
            ("clips/CLIP.mp4", true),
            ("clips/clip.Mp4", true),
            ("apps/android/release.key", true),
            (".ssh/id_rsa", true),
            ("core/fixtures/private/anything.bin", true),
            ("tools/.direnv/cache", true),
            ("apps/android/app/google-services.json", true),
            ("apps/site/.env.production", true),
            ("apps/site/.env.example", false),
            ("keys/signing.pub", false),
            ("core/fixtures/README.md", false),
            ("docs/private-notes.md", false),
        ]
    );
}

#[test]
fn a_wildcard_the_matcher_does_not_understand_is_refused() {
    // Act
    let outcome = rules_in("*.mp[34]\n");

    // Assert
    assert!(outcome.is_err(), "got {outcome:?}");
}

#[test]
fn the_glob_handles_every_position_of_the_star() {
    // Act
    let verdicts = [
        glob_matches("*.mp4", "clip.mp4"),
        glob_matches("*.mp4", "clip.mp4.txt"),
        glob_matches("id_*", "id_ed25519"),
        glob_matches("a*b*c", "axxbyyc"),
        glob_matches("a*b*c", "axxbyy"),
        glob_matches("exact", "exact"),
        glob_matches("exact", "exactly"),
        glob_matches("*", ""),
    ];

    // Assert
    assert_eq!(verdicts, [true, false, true, true, false, true, false, true]);
}
