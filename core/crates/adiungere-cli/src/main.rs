//! The adiungere command line.
//!
//! It inspects a recording, fingerprints it, finds recordings in a library, compares a manifest with a
//! file, renders a manifest as a report, and reads the project's evidence register. Everything else in the
//! roadmap arrives with the milestone that gives it something to do, because a subcommand that prints a
//! placeholder is worse than a subcommand that does not exist.
//!
//! Exit statuses are part of the contract: 0 when the answer is yes, 1 when the answer is no, 2 when the
//! question could not be asked. Every answer is rendered in full before a byte is written, and a reader that
//! closes the pipe early gets a quiet exit rather than a panic, because a pipeline that pages through a
//! listing has not asked a question the command failed to answer.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use adiungere_cli::evidence::{Probe, ProbeId, Register, Verdict};
use adiungere_cli::media::{self, MediaFailure, Output, Rendered};
use adiungere_cli::repository;
use adiungere_cli::roadmap::{MilestoneId, Roadmap};
use clap::{Args, Parser, Subcommand, ValueEnum};

fn main() -> ExitCode {
    match run() {
        Ok(answer) => emit(&answer),
        Err(failure) => {
            eprintln!("adiungere: {failure}");
            ExitCode::from(2)
        },
    }
}

/// What a person asked the command line to do.
#[derive(Debug, Parser)]
#[command(
    name = "adiungere",
    version,
    about = "Find, play, export and check multi-stream dashcam recordings.",
    long_about = "adiungere reads dashcam recordings that hold more than one camera in a single file.\n\n\
                  It inspects a recording's structure without touching its media, fingerprints every track \
                  so a stranger can reproduce the numbers with ordinary tools, finds recordings in a \
                  library, compares a manifest with a file and reports the findings, and never returns a \
                  verdict about what a recording shows."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Describe a recording's structure, reading its headers and never its media.
    Inspect {
        /// The recording.
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// How to print the answer.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },

    /// Read every byte of a recording and compute every fingerprint, producing its manifest.
    Fingerprint {
        /// The recording.
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Write the manifest to this file as well as printing the answer.
        #[arg(long, value_name = "PATH")]
        manifest: Option<PathBuf>,

        /// How to print the answer.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },

    /// Find recordings in files and directories, pairing the files of one-file-per-camera recorders.
    Detect {
        /// Files and directories to scan; a directory is scanned recursively.
        #[arg(value_name = "PATH", required = true)]
        paths: Vec<PathBuf>,

        /// Keep probes in this file so a library is not read twice.
        #[arg(long, value_name = "PATH")]
        cache: Option<PathBuf>,

        /// How to print the answer.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },

    /// Compare a manifest with a file, subject by subject.
    Verify {
        /// The manifest.
        #[arg(value_name = "MANIFEST")]
        manifest: PathBuf,

        /// The file to compare it with.
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// How to print the answer.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },

    /// Render a manifest as a report, with the commands that reproduce each number.
    Report {
        /// The manifest.
        #[arg(value_name = "MANIFEST")]
        manifest: PathBuf,

        /// How to print the answer.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },

    /// Read the evidence register.
    Probes(Probes),
}

#[derive(Debug, Args)]
struct Probes {
    #[command(subcommand)]
    action: ProbeAction,

    /// Read the register from this file instead of finding it in the repository.
    #[arg(long, value_name = "PATH", global = true)]
    register: Option<PathBuf>,

    /// Read the roadmap from this file instead of finding it in the repository.
    #[arg(long, value_name = "PATH", global = true)]
    roadmap: Option<PathBuf>,

    /// How to print the answer.
    #[arg(long, value_enum, default_value_t = Format::Text, global = true)]
    format: Format,
}

#[derive(Debug, Subcommand)]
enum ProbeAction {
    /// List the probes, most recent identifier last.
    List {
        /// Only probes carrying this verdict.
        #[arg(long, value_name = "VERDICT")]
        verdict: Option<String>,

        /// Only probes assigned to this milestone.
        #[arg(long, value_name = "MILESTONE")]
        milestone: Option<String>,
    },

    /// Show one probe in full.
    Show {
        /// The identifier, such as P11.
        #[arg(value_name = "IDENTIFIER")]
        id: String,
    },

    /// Check that the register and the roadmap agree.
    Check,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// Aligned columns, for a person.
    Text,
    /// One JSON document, for a pipeline.
    Json,
}

/// A rendered answer and the status that goes with it.
#[derive(Debug)]
struct Answer {
    text: String,
    code: ExitCode,
}

impl Answer {
    fn yes(text: String) -> Self {
        Self {
            text,
            code: ExitCode::SUCCESS,
        }
    }

    fn no(text: String) -> Self {
        Self {
            text,
            code: ExitCode::FAILURE,
        }
    }
}

/// Anything that stops the command line before it can answer.
#[derive(Debug)]
enum Failure {
    NoRepository,
    Unreadable {
        path: PathBuf,
        cause: std::io::Error,
    },
    Register {
        path: PathBuf,
        cause: adiungere_cli::evidence::ParseError,
    },
    Roadmap {
        path: PathBuf,
        cause: adiungere_cli::roadmap::ParseError,
    },
    UnknownVerdict {
        value: String,
    },
    UnknownMilestone {
        value: String,
    },
    UnknownIdentifier {
        value: String,
    },
    NoSuchProbe {
        id: ProbeId,
    },
    Serialisation {
        cause: serde_json::Error,
    },
    Media(MediaFailure),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Media(cause) => write!(formatter, "{cause}"),
            Self::NoRepository => formatter.write_str(
                "no repository found above the working directory. Run this inside a checkout, or pass \
                 --register (and --roadmap for a check)",
            ),
            Self::Unreadable { path, cause } => {
                write!(formatter, "cannot read {}: {cause}", path.display())
            },
            Self::Register { path, cause } => {
                write!(formatter, "{}: {cause}", path.display())
            },
            Self::Roadmap { path, cause } => {
                write!(formatter, "{}: {cause}", path.display())
            },
            Self::UnknownVerdict { value } => write!(
                formatter,
                "\"{value}\" is not a verdict; use measured, reasoned, unavailable or not started"
            ),
            Self::UnknownMilestone { value } => {
                write!(formatter, "\"{value}\" is not a milestone identifier such as M1")
            },
            Self::UnknownIdentifier { value } => {
                write!(
                    formatter,
                    "\"{value}\" is not a probe identifier such as P11; two or more digits, never zero"
                )
            },
            Self::NoSuchProbe { id } => write!(formatter, "the register holds no {id}"),
            Self::Serialisation { cause } => write!(formatter, "cannot write the answer: {cause}"),
        }
    }
}

/// Writes an answer to standard output and turns it into a process status.
///
/// A closed pipe is not a failure of this command: the reader stopped reading. Anything else that stops the
/// write is reported on the error stream with the usage status, because an answer that was computed and
/// never delivered must not look like a yes.
fn emit(answer: &Answer) -> ExitCode {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    let written = handle
        .write_all(answer.text.as_bytes())
        .and_then(|()| handle.flush());

    match written {
        Ok(()) => answer.code,
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("adiungere: cannot write the answer: {error}");
            ExitCode::from(2)
        },
    }
}

fn run() -> Result<Answer, Failure> {
    let cli = Cli::parse();

    match cli.command {
        Command::Inspect { file, format } => media::inspect(&file, output(format))
            .map(answer)
            .map_err(Failure::Media),
        Command::Fingerprint {
            file,
            manifest,
            format,
        } => media::fingerprint(&file, output(format), manifest.as_deref())
            .map(answer)
            .map_err(Failure::Media),
        Command::Detect { paths, cache, format } => media::detect(&paths, output(format), cache.as_deref())
            .map(answer)
            .map_err(Failure::Media),
        Command::Verify {
            manifest,
            file,
            format,
        } => media::verify(&manifest, &file, output(format))
            .map(answer)
            .map_err(Failure::Media),
        Command::Report { manifest, format } => media::report(&manifest, output(format))
            .map(answer)
            .map_err(Failure::Media),
        Command::Probes(probes) => run_probes(probes),
    }
}

const fn output(format: Format) -> Output {
    match format {
        Format::Text => Output::Text,
        Format::Json => Output::Json,
    }
}

fn answer(rendered: Rendered) -> Answer {
    if rendered.negative {
        Answer::no(rendered.text)
    } else {
        Answer::yes(rendered.text)
    }
}

fn run_probes(probes: Probes) -> Result<Answer, Failure> {
    let mut sources = Sources::new(probes.register, probes.roadmap);
    let register = read_register(&sources.register()?)?;

    match probes.action {
        ProbeAction::List { verdict, milestone } => {
            let verdict = verdict
                .map(|value| {
                    value
                        .parse::<Verdict>()
                        .map_err(|()| Failure::UnknownVerdict { value: value.clone() })
                })
                .transpose()?;

            let milestone = milestone
                .map(|value| {
                    value
                        .parse::<MilestoneId>()
                        .map_err(|()| Failure::UnknownMilestone { value: value.clone() })
                })
                .transpose()?;

            list(&register, verdict, milestone, probes.format)
        },

        ProbeAction::Show { id } => {
            let id = ProbeId::canonical(&id).ok_or(Failure::UnknownIdentifier { value: id })?;
            let probe = register.get(id).ok_or(Failure::NoSuchProbe { id })?;

            show(probe, probes.format)
        },

        ProbeAction::Check => {
            let roadmap = read_roadmap(&sources.roadmap()?)?;
            check(&register, &roadmap, probes.format)
        },
    }
}

/// Where the two documents are read from, resolved only when a command needs them.
///
/// A listing never opens the roadmap, so a person who passes `--register` alone from outside a checkout gets
/// their listing rather than a complaint about a file the command was not going to read.
#[derive(Debug)]
struct Sources {
    register: Option<PathBuf>,
    roadmap: Option<PathBuf>,
    root: Option<PathBuf>,
}

impl Sources {
    fn new(register: Option<PathBuf>, roadmap: Option<PathBuf>) -> Self {
        Self {
            register,
            roadmap,
            root: None,
        }
    }

    fn register(&mut self) -> Result<PathBuf, Failure> {
        match self.register.clone() {
            Some(path) => Ok(path),
            None => Ok(self.root()?.join(repository::REGISTER)),
        }
    }

    fn roadmap(&mut self) -> Result<PathBuf, Failure> {
        match self.roadmap.clone() {
            Some(path) => Ok(path),
            None => Ok(self.root()?.join(repository::ROADMAP)),
        }
    }

    fn root(&mut self) -> Result<PathBuf, Failure> {
        if let Some(root) = &self.root {
            return Ok(root.clone());
        }

        let here = std::env::current_dir().map_err(|cause| Failure::Unreadable {
            path: PathBuf::from("."),
            cause,
        })?;
        let root = repository::locate_root(&here).ok_or(Failure::NoRepository)?;
        self.root = Some(root.clone());

        Ok(root)
    }
}

fn read_register(path: &Path) -> Result<Register, Failure> {
    let source = std::fs::read_to_string(path).map_err(|cause| Failure::Unreadable {
        path: path.to_path_buf(),
        cause,
    })?;

    Register::parse(&source).map_err(|cause| Failure::Register {
        path: path.to_path_buf(),
        cause,
    })
}

fn read_roadmap(path: &Path) -> Result<Roadmap, Failure> {
    let source = std::fs::read_to_string(path).map_err(|cause| Failure::Unreadable {
        path: path.to_path_buf(),
        cause,
    })?;

    Roadmap::parse(&source).map_err(|cause| Failure::Roadmap {
        path: path.to_path_buf(),
        cause,
    })
}

fn list(
    register: &Register,
    verdict: Option<Verdict>,
    milestone: Option<MilestoneId>,
    format: Format,
) -> Result<Answer, Failure> {
    let selected: Vec<&Probe> = register
        .probes()
        .iter()
        .filter(|probe| verdict.is_none_or(|wanted| probe.verdict == wanted))
        .filter(|probe| milestone.is_none_or(|wanted| probe.milestone == wanted))
        .collect();

    if format == Format::Json {
        let rendered =
            serde_json::to_string_pretty(&selected).map_err(|cause| Failure::Serialisation { cause })?;
        return Ok(Answer::yes(format!("{rendered}\n")));
    }

    let widest = selected
        .iter()
        .map(|probe| probe.verdict.as_str().len())
        .max()
        .unwrap_or(0);

    let mut text = String::new();

    for probe in &selected {
        let _ = writeln!(
            text,
            "{id}  {verdict:width$}  {milestone:>3}  {title}",
            id = probe.id,
            verdict = probe.verdict.as_str(),
            milestone = probe.milestone.to_string(),
            title = probe.title,
            width = widest,
        );
    }

    text.push('\n');
    text.push_str(&summary(register, selected.len()));
    text.push('\n');

    Ok(Answer::yes(text))
}

fn summary(register: &Register, shown: usize) -> String {
    let total = register.probes().len();
    let mut line = if shown == total {
        format!("{total} probes: ")
    } else {
        format!("{shown} of {total} probes shown. Across the whole register: ")
    };

    let counts = register.tally();

    for (index, (verdict, count)) in counts.iter().enumerate() {
        if index > 0 {
            line.push_str(", ");
        }

        let _ = write!(line, "{count} {verdict}");
    }

    line.push('.');
    line
}

fn show(probe: &Probe, format: Format) -> Result<Answer, Failure> {
    if format == Format::Json {
        let rendered =
            serde_json::to_string_pretty(probe).map_err(|cause| Failure::Serialisation { cause })?;
        return Ok(Answer::yes(format!("{rendered}\n")));
    }

    let mut text = String::new();

    let _ = writeln!(text, "{} {}", probe.id, probe.title);
    text.push('\n');
    let _ = writeln!(text, "  Verdict    {}", probe.verdict);
    let _ = writeln!(text, "  Milestone  {}", probe.milestone);
    text.push('\n');
    push_paragraph(&mut text, "Question", &probe.question);
    push_paragraph(&mut text, "Method", &probe.method);
    push_paragraph(&mut text, "Decides", &probe.decides);

    match &probe.result {
        Some(result) => push_paragraph(&mut text, "Result", result),
        None => {
            let _ = writeln!(
                text,
                "No result is recorded, because the verdict is \"{}\". That is not evidence of anything.",
                probe.verdict
            );
        },
    }

    Ok(Answer::yes(text))
}

/// How wide a paragraph is allowed to be when printed for a person.
const WRAP_AT: usize = 96;

fn push_paragraph(text: &mut String, heading: &str, body: &str) {
    text.push_str(heading);
    text.push('\n');

    for line in body.lines() {
        if line.trim().is_empty() {
            text.push('\n');
            continue;
        }

        for wrapped in wrap(line, WRAP_AT) {
            text.push_str("  ");
            text.push_str(&wrapped);
            text.push('\n');
        }
    }

    text.push('\n');
}

/// Breaks a line at word boundaries, leaving a word longer than the width on its own line.
fn wrap(line: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in line.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut current));
        }

        if !current.is_empty() {
            current.push(' ');
        }

        current.push_str(word);
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn check(register: &Register, roadmap: &Roadmap, format: Format) -> Result<Answer, Failure> {
    let discrepancies = register.reconcile(roadmap);
    let agree = discrepancies.is_empty();

    if format == Format::Json {
        let rendered =
            serde_json::to_string_pretty(&discrepancies).map_err(|cause| Failure::Serialisation { cause })?;
        let text = format!("{rendered}\n");

        return Ok(if agree {
            Answer::yes(text)
        } else {
            Answer::no(text)
        });
    }

    if agree {
        let milestones = roadmap.sections().len();
        let plural = if milestones == 1 { "" } else { "s" };

        return Ok(Answer::yes(format!(
            "The register and the roadmap agree: {} probes across {milestones} milestone{plural}.\n",
            register.probes().len(),
        )));
    }

    let mut text = String::from("The register and the roadmap disagree:\n");

    for discrepancy in &discrepancies {
        let _ = writeln!(text, "  {discrepancy}");
    }

    text.push('\n');
    text.push_str(
        "A probe nobody can find from the roadmap is a question nobody will answer. Name it in its \
         milestone, or move it to the milestone that carries it.\n",
    );

    Ok(Answer::no(text))
}
